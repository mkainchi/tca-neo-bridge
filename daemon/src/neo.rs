//! NEO report 01 encoding and the TCA-to-NEO mapping (port of tca_to_neo.py / contract_probe.py).
use crate::config::{Config, Device, Direction, HatMode, Lever, SplitHighG};

pub const REPORT_LEN: usize = 64;
pub const STICK_LEN: usize = 64;
pub const QUADRANT_LEN: usize = 44;
pub type Report = [u8; REPORT_LEN];

/// POV positions 0..7 as (x, y); 8 is neutral.
pub const HATS: [(i8, i8); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// Report 01: POV low nibble of byte 1, buttons bits 4..18 of bytes 1..3, seven LE axes at byte 4.
pub fn input_report(axes: [u16; 7], buttons: u16, hat: u8) -> Report {
    let mut report = [0u8; REPORT_LEN];
    report[0] = 1;
    let packed = u32::from(hat.min(8)) | u32::from(buttons & 0x7fff) << 4;
    report[1..4].copy_from_slice(&packed.to_le_bytes()[..3]);
    for (index, axis) in axes.iter().enumerate() {
        report[4 + 2 * index..6 + 2 * index].copy_from_slice(&axis.to_le_bytes());
    }
    report
}

pub fn neutral() -> Report {
    input_report([32768; 7], 0, 8)
}

pub fn centered(value: f64, deadzone: f64) -> f64 {
    let value = value.clamp(-1.0, 1.0);
    if value.abs() <= deadzone {
        return 0.0;
    }
    ((value.abs() - deadzone) / (1.0 - deadzone)).copysign(value)
}

/// Python round() parity: ties to even.
pub fn unit16(value: f64) -> u16 {
    ((value.clamp(-1.0, 1.0) + 1.0) * 65535.0 / 2.0).round_ties_even() as u16
}

/// Last meaningful movement wins; simultaneous samples keep ownership (app/model.py).
#[derive(Debug, Clone)]
pub struct ThrottleSelector {
    previous: Option<[f64; 2]>,
    active: usize,
}

impl Default for ThrottleSelector {
    fn default() -> Self {
        Self { previous: None, active: 1 }
    }
}

impl ThrottleSelector {
    pub fn read(&mut self, stick: f64, quadrant: f64) -> f64 {
        let values = [stick, quadrant];
        let previous = self.previous.get_or_insert(values);
        let moved: Vec<usize> = (0..2)
            .filter(|&index| {
                let changed = (values[index] - previous[index]).abs() > 0.02;
                previous[index] = if changed { values[index] } else { previous[index] };
                changed
            })
            .collect();
        if moved.len() == 1 {
            self.active = moved[0];
        }
        values[self.active]
    }
}

pub const HAT_NAMES: [&str; 8] = ["up", "up-right", "right", "down-right", "down", "down-left", "left", "up-left"];

/// Sidestick hat direction (byte 7 low nibble), None when centred.
pub fn hat(stick: &[u8]) -> Option<&'static str> {
    HAT_NAMES.get(usize::from(stick[7] & 15)).copied()
}

/// Quadrant left and right lever travel, 0.0 at idle to 1.0 fully forward. The TCA reports forward as the low raw
/// value (idle 65535, full forward 0), which is also the NEO Z convention, so the throttle axis itself uses raw values.
pub fn levers(quadrant: &[u8]) -> [f64; 2] {
    [3, 5].map(|offset| 1.0 - f64::from(u16::from_le_bytes([quadrant[offset], quadrant[offset + 1]])) / 65535.0)
}

/// One lever at or above the forward threshold and the other at or below the back threshold, in either order.
pub fn split(rule: &SplitHighG, levers: [f64; 2]) -> bool {
    rule.enabled && levers[0].max(levers[1]) >= rule.forward && levers[0].min(levers[1]) <= rule.back
}

/// Pressed TCA sources as config keys ("s1".."s17", "q1".."q31").
pub fn pressed(stick: &[u8], quadrant: &[u8]) -> Vec<String> {
    let (stick_mask, quadrant_mask) = masks(stick, quadrant);
    let mut result: Vec<String> = (0..17).filter(|i| stick_mask & 1 << i != 0).map(|i| format!("s{}", i + 1)).collect();
    result.extend((0..31).filter(|i| quadrant_mask & 1 << i != 0).map(|i| format!("q{}", i + 1)));
    result
}

fn masks(stick: &[u8], quadrant: &[u8]) -> (u32, u32) {
    let stick_mask = u32::from_le_bytes([stick[8], stick[9], stick[10], 0]) & 0x1ffff;
    let quadrant_mask = u32::from(u16::from_le_bytes([quadrant[1], quadrant[2]]))
        | u32::from(quadrant[9] & 15) << 16
        | (u32::from(u16::from_le_bytes([quadrant[12], quadrant[13]])) & 0x7ff) << 20;
    (stick_mask, quadrant_mask)
}

fn valid(report: Option<&[u8]>, length: usize) -> Option<&[u8]> {
    report.filter(|r| r.len() == length && r[0] == 1)
}

#[derive(Debug, Clone)]
pub struct Mapper {
    config: Config,
    stick_targets: [u16; 17],
    quadrant_targets: [u16; 31],
    stick_pov: [Option<Direction>; 17],
    quadrant_pov: [Option<Direction>; 31],
    hat_targets: [u16; 4],
    hat_pov: [Option<Direction>; 4],
    throttle: ThrottleSelector,
}

impl Mapper {
    /// The config must already be validated (config::validate).
    pub fn new(config: &Config) -> Self {
        let mut mapper = Self {
            config: config.clone(),
            stick_targets: [0; 17],
            quadrant_targets: [0; 31],
            stick_pov: [None; 17],
            quadrant_pov: [None; 31],
            hat_targets: [0; 4],
            hat_pov: [None; 4],
            throttle: ThrottleSelector::default(),
        };
        for (key, targets) in &config.buttons {
            let mask = targets.iter().fold(0u16, |mask, target| mask | 1 << (target - 1));
            match crate::config::source(key).expect("validated source") {
                (Device::Stick, index) => mapper.stick_targets[index] |= mask,
                (Device::Quadrant, index) => mapper.quadrant_targets[index] |= mask,
                (Device::Hat, index) => mapper.hat_targets[index] |= mask,
            }
        }
        for (key, direction) in &config.pov {
            match crate::config::source(key).expect("validated source") {
                (Device::Stick, index) => mapper.stick_pov[index] = Some(*direction),
                (Device::Quadrant, index) => mapper.quadrant_pov[index] = Some(*direction),
                (Device::Hat, index) => mapper.hat_pov[index] = Some(*direction),
            }
        }
        mapper
    }

    fn flight_axis(&self, raw: u32, maximum: u32, invert: bool) -> u16 {
        let raw = raw.min(maximum);
        if self.config.deadzone == 0.0 {
            let result = (raw * 65535 / maximum) as u16;
            return if invert { 65535 - result } else { result };
        }
        let value = centered(2.0 * f64::from(raw) / f64::from(maximum) - 1.0, self.config.deadzone);
        unit16(if invert { -value } else { value })
    }

    /// Missing or malformed input resets throttle ownership and yields the neutral report.
    pub fn convert(&mut self, stick: Option<&[u8]>, quadrant: Option<&[u8]>) -> Report {
        let (Some(stick), Some(quadrant)) = (valid(stick, STICK_LEN), valid(quadrant, QUADRANT_LEN)) else {
            self.throttle = ThrottleSelector::default();
            return neutral();
        };
        let invert = &self.config.invert;
        let roll = u32::from(u16::from_le_bytes([stick[1], stick[2]]));
        let pitch = u32::from(u16::from_le_bytes([stick[3], stick[4]]));
        let lever_offset = match self.config.throttle_lever {
            Lever::Left => 3,
            Lever::Right => 5,
        };
        let lever = u16::from_le_bytes([quadrant[lever_offset], quadrant[lever_offset + 1]]);
        let active = self.throttle.read(2.0 * f64::from(stick[6]) / 255.0 - 1.0, 2.0 * f64::from(lever) / 65535.0 - 1.0);
        let signed = |value: f64| if invert.throttle { -value } else { value };
        let positions = levers(quadrant);
        // Split High-G: the throttle follows the forward lever (the greater travel), converted back to the raw axis
        // convention (2 * raw - 1 = 1 - 2 * travel) with the same inversion as normal throttle; NEO 5 is held below.
        let split = split(&self.config.split_high_g, positions);
        let throttle = if split { unit16(signed(1.0 - 2.0 * positions[0].max(positions[1]))) } else { unit16(signed(active)) };
        let hat = HATS.get(usize::from(stick[7] & 15)).copied().unwrap_or((0, 0));
        // Camera mode: the hat moves the mini-stick. Mapped mode: the camera rests and hat directions act as sources.
        let mapped = self.config.hat == HatMode::Mapped;
        let camera = if mapped { (0, 0) } else { hat };
        let held = [hat.1 < 0, hat.0 > 0, hat.1 > 0, hat.0 < 0].map(|on| on && mapped);
        let strength = self.config.camera_strength;
        let axes = [
            self.flight_axis(roll, 16383, invert.roll),
            self.flight_axis(pitch, 16383, invert.pitch),
            throttle,
            unit16(f64::from(camera.0) * strength),
            unit16(f64::from(camera.1) * strength),
            self.flight_axis(u32::from(stick[5]), 255, invert.yaw),
            32768,
        ];
        let (stick_mask, quadrant_mask) = masks(stick, quadrant);
        let mut buttons = u16::from(split) << 4;
        let mut order = (0i8, 0i8);
        let sources = (0..17)
            .filter(|i| stick_mask & 1 << i != 0)
            .map(|i| (self.stick_targets[i], self.stick_pov[i]))
            .chain((0..31).filter(|i| quadrant_mask & 1 << i != 0).map(|i| (self.quadrant_targets[i], self.quadrant_pov[i])))
            .chain((0..4).filter(|&i| held[i]).map(|i| (self.hat_targets[i], self.hat_pov[i])));
        for (targets, direction) in sources {
            buttons |= targets;
            let (x, y) = direction.map_or((0, 0), Direction::vector);
            order = (order.0 + x, order.1 + y);
        }
        let order = (order.0.clamp(-1, 1), order.1.clamp(-1, 1));
        let hat = HATS.iter().position(|&h| h == order).map_or(8, |p| p as u8);
        input_report(axes, buttons, hat)
    }
}
