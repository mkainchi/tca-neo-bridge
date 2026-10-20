//! NEO report 01 encoding and the TCA-to-NEO mapping (port of tca_to_neo.py / contract_probe.py).
use crate::config::{AxisTuning, Config, Device, Direction, HatMode, Lever, Mode, SplitHighG, Throttle, ThrottleMode};
use serde::Serialize;
use std::time::Duration;

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

    /// 0 while the sidestick slider owns the throttle, 1 while the quadrant lever does.
    pub fn active(&self) -> usize {
        self.active
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

/// Mappable sources in one index space: sidestick 0..17, quadrant 17..48, hat directions 48..52.
const SOURCES: usize = 52;

fn slot(key: &str) -> usize {
    match crate::config::source(key).expect("validated source") {
        (Device::Stick, index) => index,
        (Device::Quadrant, index) => 17 + index,
        (Device::Hat, index) => 48 + index,
    }
}

/// Throttle values this close to centre (or a move across it) arm a safe start.
pub const ARM_BAND: f64 = 0.05;

/// One axis value in -1..1 through deadzone, saturation, curve and anti-deadzone (smoothing is stateful, in the Mapper).
pub fn shape(value: f64, tuning: &AxisTuning) -> f64 {
    let value = value.clamp(-1.0, 1.0);
    if value.abs() <= tuning.deadzone {
        return 0.0;
    }
    let travel = ((value.abs() - tuning.deadzone) / (tuning.saturation - tuning.deadzone)).min(1.0);
    let k = tuning.curve;
    let curved = if k >= 0.0 { (1.0 - k) * travel + k * travel.powi(3) } else { (1.0 + k) * travel - k * travel.cbrt() };
    (tuning.anti_deadzone + (1.0 - tuning.anti_deadzone) * curved).copysign(value)
}

/// Detent throttle: lever travel (0 idle, 1 forward) to -1 full brake, 0 cruise, +1 full boost.
pub fn detent(travel: f64, throttle: &Throttle) -> f64 {
    let (low, high) = (throttle.cruise - throttle.band, throttle.cruise + throttle.band);
    if travel <= throttle.idle {
        -1.0
    } else if travel < low {
        (travel - throttle.idle) / (low - throttle.idle) - 1.0
    } else if travel <= high {
        0.0
    } else if travel < throttle.boost {
        (travel - high) / (throttle.boost - high)
    } else {
        1.0
    }
}

/// Side effects the emulation loop applies; the report itself stays a pure function of the inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    TogglePause,
}

/// Raw inputs (axes -1..1, lever travel 0..1) next to what the NEO receives, for the page's live previews.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Monitor {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub slider: f64,
    pub levers: [f64; 2],
    pub axes: [u16; 7],
    pub armed: bool,
    pub precision: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct SourceState {
    down: bool,
    until: Duration,
    latched: bool,
}

#[derive(Debug, Clone)]
pub struct Mapper {
    config: Config,
    targets: [u16; SOURCES],
    pov: [Option<Direction>; SOURCES],
    modes: [Mode; SOURCES],
    states: [SourceState; SOURCES],
    /// False until the first complete frame: sources already down then (a latched switch) are not edges.
    primed: bool,
    precision: Option<usize>,
    pause: u64,
    pause_held: bool,
    throttle: ThrottleSelector,
    armed: bool,
    start_side: Option<bool>,
    smoothed: [Option<f64>; 3],
    now: Duration,
    events: Vec<Event>,
    monitor: Option<Monitor>,
}

impl Mapper {
    /// The config must already be validated (config::validate).
    pub fn new(config: &Config) -> Self {
        let mut mapper = Self {
            config: config.clone(),
            targets: [0; SOURCES],
            pov: [None; SOURCES],
            modes: [Mode::Hold; SOURCES],
            states: [SourceState::default(); SOURCES],
            primed: false,
            precision: config.precision.source.as_deref().map(slot),
            pause: config.combos.pause.iter().fold(0, |mask, key| mask | 1 << slot(key)),
            pause_held: false,
            throttle: ThrottleSelector::default(),
            armed: !config.safe_start,
            start_side: None,
            smoothed: [None; 3],
            now: Duration::ZERO,
            events: Vec::new(),
            monitor: None,
        };
        for (key, targets) in &config.buttons {
            mapper.targets[slot(key)] |= targets.iter().fold(0u16, |mask, target| mask | 1 << (target - 1));
        }
        for (key, direction) in &config.pov {
            mapper.pov[slot(key)] = Some(*direction);
        }
        for (key, mode) in &config.behaviours {
            mapper.modes[slot(key)] = *mode;
        }
        mapper
    }

    /// New settings mid-flight: keep throttle ownership, safe-start arming, smoothing and source states, so saving
    /// or previewing never fires a latched switch or drops the throttle.
    pub fn reconfigure(&mut self, config: &Config) {
        let mut next = Mapper::new(config);
        next.states = self.states;
        next.primed = self.primed;
        next.pause_held = self.pause_held;
        next.throttle = self.throttle.clone();
        next.armed |= self.armed;
        next.start_side = self.start_side;
        next.smoothed = self.smoothed;
        next.now = self.now;
        *self = next;
    }

    /// Time of the next frame, for pulses (any monotonic clock).
    pub fn at(&mut self, now: Duration) -> &mut Self {
        self.now = now;
        self
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn monitor(&self) -> Option<Monitor> {
        self.monitor
    }

    fn flight_axis(&mut self, axis: usize, raw: u32, maximum: u32, invert: bool, scale: f64) -> u16 {
        let raw = raw.min(maximum);
        let tuning = [self.config.axes.roll, self.config.axes.pitch, self.config.axes.yaw][axis];
        if tuning == (AxisTuning { deadzone: 0.0, saturation: 1.0, anti_deadzone: 0.0, curve: 0.0, smoothing: 0.0 }) && scale == 1.0 {
            let result = (raw * 65535 / maximum) as u16;
            return if invert { 65535 - result } else { result };
        }
        let value = shape(2.0 * f64::from(raw) / f64::from(maximum) - 1.0, &tuning) * scale;
        let value = self.smoothed[axis].map_or(value, |previous| previous * tuning.smoothing + value * (1.0 - tuning.smoothing));
        self.smoothed[axis] = Some(value);
        unit16(if invert { -value } else { value })
    }

    /// Whether a source drives its targets this frame, by its behaviour.
    fn active(&mut self, slot: usize, down: bool) -> bool {
        let (now, pulse) = (self.now, Duration::from_millis(u64::from(self.config.timing.pulse_ms)));
        let state = &mut self.states[slot];
        let (rising, changed) = (down && !state.down, down != state.down);
        state.down = down;
        match self.modes[slot] {
            Mode::Hold => down,
            Mode::Toggle => {
                state.latched ^= rising;
                state.latched
            }
            mode => {
                if rising || (mode == Mode::PulseChange && changed) {
                    state.until = now + pulse;
                }
                now < state.until
            }
        }
    }

    /// Raw-convention throttle (-1 forward … +1 idle) for a quadrant lever's travel.
    fn lever_throttle(&self, travel: f64) -> f64 {
        match self.config.throttle.mode {
            ThrottleMode::Linear => 1.0 - 2.0 * travel,
            ThrottleMode::Detents => -detent(travel, &self.config.throttle),
        }
    }

    /// Missing or malformed input resets throttle ownership, disarms a safe start and yields the neutral report.
    pub fn convert(&mut self, stick: Option<&[u8]>, quadrant: Option<&[u8]>) -> Report {
        let (Some(stick), Some(quadrant)) = (valid(stick, STICK_LEN), valid(quadrant, QUADRANT_LEN)) else {
            self.throttle = ThrottleSelector::default();
            self.armed = !self.config.safe_start;
            self.start_side = None;
            self.primed = false;
            self.monitor = None;
            return neutral();
        };
        let invert = self.config.invert.clone();
        let hat = HATS.get(usize::from(stick[7] & 15)).copied().unwrap_or((0, 0));
        // Camera mode: the hat moves the mini-stick. Mapped mode: the camera rests and hat directions act as sources.
        let mapped = self.config.hat == HatMode::Mapped;
        let camera = if mapped { (0, 0) } else { hat };
        let held = [hat.1 < 0, hat.0 > 0, hat.1 > 0, hat.0 < 0].map(|on| on && mapped);
        let (stick_mask, quadrant_mask) = masks(stick, quadrant);
        let pressed = u64::from(stick_mask) | u64::from(quadrant_mask) << 17 | held.iter().rev().fold(0u64, |mask, &on| mask << 1 | u64::from(on)) << 48;
        if !self.primed {
            for (slot, state) in self.states.iter_mut().enumerate() {
                state.down = pressed >> slot & 1 == 1;
            }
            self.pause_held = self.pause != 0 && pressed & self.pause == self.pause;
            self.primed = true;
        }
        let combo = self.pause != 0 && pressed & self.pause == self.pause;
        if combo && !self.pause_held {
            self.events.push(Event::TogglePause);
        }
        self.pause_held = combo;
        let precise = self.precision.is_some_and(|slot| pressed >> slot & 1 == 1);
        let scale = if precise { self.config.precision.scale } else { 1.0 };

        let roll = u32::from(u16::from_le_bytes([stick[1], stick[2]]));
        let pitch = u32::from(u16::from_le_bytes([stick[3], stick[4]]));
        let lever_index = match self.config.throttle_lever {
            Lever::Left => 0,
            Lever::Right => 1,
        };
        let lever = u16::from_le_bytes([quadrant[3 + 2 * lever_index], quadrant[4 + 2 * lever_index]]);
        let active = self.throttle.read(2.0 * f64::from(stick[6]) / 255.0 - 1.0, 2.0 * f64::from(lever) / 65535.0 - 1.0);
        let positions = levers(quadrant);
        // Split High-G: the throttle follows the forward lever (the greater travel), with the same response and
        // inversion as the normal throttle; NEO 5 is held below.
        let split = split(&self.config.split_high_g, positions);
        let value = if split {
            self.lever_throttle(positions[0].max(positions[1]))
        } else if self.throttle.active() == 1 && self.config.throttle.mode == ThrottleMode::Detents {
            self.lever_throttle(positions[lever_index])
        } else {
            active
        };
        // Safe start: the throttle rests at centre until the active control reaches it or moves across it.
        if !self.armed {
            let side = value > 0.0;
            let crossed = *self.start_side.get_or_insert(side) != side;
            self.armed = crossed || value.abs() <= ARM_BAND;
        }
        let throttle = if !self.armed {
            32768
        } else if invert.throttle {
            unit16(-value)
        } else {
            unit16(value)
        };
        let strength = self.config.camera_strength;
        let axes = [
            self.flight_axis(0, roll, 16383, invert.roll, scale),
            self.flight_axis(1, pitch, 16383, invert.pitch, scale),
            throttle,
            unit16(f64::from(camera.0) * strength),
            unit16(f64::from(camera.1) * strength),
            self.flight_axis(2, u32::from(stick[5]), 255, invert.yaw, scale),
            32768,
        ];
        let mut buttons = u16::from(split) << 4;
        let mut order = (0i8, 0i8);
        for slot in 0..SOURCES {
            if self.active(slot, pressed >> slot & 1 == 1) {
                buttons |= self.targets[slot];
                let (x, y) = self.pov[slot].map_or((0, 0), Direction::vector);
                order = (order.0 + x, order.1 + y);
            }
        }
        let order = (order.0.clamp(-1, 1), order.1.clamp(-1, 1));
        let hat = HATS.iter().position(|&h| h == order).map_or(8, |p| p as u8);
        let unit = |raw: u32, maximum: f64| (2.0 * f64::from(raw) / maximum - 1.0).clamp(-1.0, 1.0);
        self.monitor = Some(Monitor {
            roll: unit(roll, 16383.0),
            pitch: unit(pitch, 16383.0),
            yaw: unit(u32::from(stick[5]), 255.0),
            slider: unit(u32::from(stick[6]), 255.0),
            levers: positions,
            axes,
            armed: self.armed,
            precision: precise,
        });
        input_report(axes, buttons, hat)
    }
}
