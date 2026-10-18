//! NEO report 01 encoding and the controller-to-NEO mapping (port of tca_to_neo.py / contract_probe.py,
//! generalised from the TCA pair to any set of joysticks).
use crate::config::{Binding, Config, Control, Direction, HatMode, Profile, SplitHighG};
use crate::hid::InputState;
use crate::models::DeviceInfo;
use std::collections::BTreeMap;

pub const REPORT_LEN: usize = 64;
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

/// Last meaningful movement wins; simultaneous moves keep ownership; the last axis owns it at start (app/model.py).
#[derive(Debug, Clone, Default)]
pub struct ThrottleSelector {
    previous: Option<Vec<f64>>,
    active: Option<usize>,
}

impl ThrottleSelector {
    /// (owner index, its value); None without throttle axes.
    pub fn read(&mut self, values: &[f64]) -> Option<(usize, f64)> {
        if values.is_empty() {
            return None;
        }
        let previous = self.previous.get_or_insert_with(|| values.to_vec());
        let moved: Vec<usize> = (0..values.len())
            .filter(|&index| {
                let changed = (values[index] - previous[index]).abs() > 0.02;
                previous[index] = if changed { values[index] } else { previous[index] };
                changed
            })
            .collect();
        if moved.len() == 1 {
            self.active = Some(moved[0]);
        }
        let active = *self.active.get_or_insert(values.len() - 1);
        Some((active, values[active]))
    }
}

pub const HAT_NAMES: [&str; 8] = ["up", "up-right", "right", "down-right", "down", "down-left", "left", "up-left"];

/// One lever split rule check: one lever at or above the forward threshold and the other at or below the back one.
pub fn split(rule: &SplitHighG, levers: [f64; 2]) -> bool {
    rule.enabled && levers[0].max(levers[1]) >= rule.forward && levers[0].min(levers[1]) <= rule.back
}

/// Pressed sources as profile keys, per device in order: buttons, then the hat's cardinal components
/// ("tca-stick/b1", "tca-stick/hat-up"), matching the mappable hat sources.
pub fn pressed(devices: &[DeviceInfo], inputs: &[Option<InputState>]) -> Vec<String> {
    let mut result = Vec::new();
    for (device, state) in devices.iter().zip(inputs) {
        let Some(state) = state else { continue };
        result.extend((0..128).filter(|i| state.buttons & 1 << i != 0).map(|i| format!("{}/b{}", device.id, i + 1)));
        let hat = state.hat.map_or("", |h| HAT_NAMES[usize::from(h)]);
        result.extend(hat.split('-').filter(|d| !d.is_empty()).map(|d| format!("{}/hat-{d}", device.id)));
    }
    result
}

/// Live axis positions per device, -1 at the low raw end to 1 at the high end.
pub fn axes(devices: &[DeviceInfo], inputs: &[Option<InputState>]) -> BTreeMap<String, Vec<f64>> {
    devices
        .iter()
        .zip(inputs)
        .filter_map(|(device, state)| {
            let state = state.as_ref()?;
            Some((device.id.clone(), device.layout.axes.iter().zip(&state.axes).map(|(a, &v)| 2.0 * f64::from(v.min(a.max)) / f64::from(a.max) - 1.0).collect()))
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct Bound {
    device: usize,
    axis: usize,
    max: u32,
    invert: bool,
}

#[derive(Debug, Clone)]
struct Slot {
    targets: Vec<u16>,
    pov: Vec<Option<Direction>>,
    hat_targets: [u16; 4],
    hat_pov: [Option<Direction>; 4],
}

#[derive(Debug, Clone)]
pub struct Mapper {
    deadzone: f64,
    strength: f64,
    hat: HatMode,
    camera: Option<usize>,
    slots: Vec<Slot>,
    roll: Option<Bound>,
    pitch: Option<Bound>,
    yaw: Option<Bound>,
    throttle: Vec<Bound>,
    rule: SplitHighG,
    levers: Option<[Bound; 2]>,
    selector: ThrottleSelector,
}

fn resolve(devices: &[DeviceInfo], binding: &Binding) -> Option<Bound> {
    let device = devices.iter().position(|d| d.id == binding.device)?;
    let axis = devices[device].layout.axis(&binding.axis)?;
    Some(Bound { device, axis, max: devices[device].layout.axes[axis].max.max(1), invert: binding.invert })
}

impl Mapper {
    /// `devices` are the session's devices in input order. The config must already be validated; bindings and
    /// sources naming absent devices, axes or buttons are ignored.
    pub fn new(config: &Config, devices: &[DeviceInfo]) -> Self {
        Self::with_profile(config, &config.profile(devices), devices)
    }

    pub fn with_profile(config: &Config, profile: &Profile, devices: &[DeviceInfo]) -> Self {
        let mut slots: Vec<Slot> = devices
            .iter()
            .map(|d| Slot { targets: vec![0; usize::from(d.layout.buttons)], pov: vec![None; usize::from(d.layout.buttons)], hat_targets: [0; 4], hat_pov: [None; 4] })
            .collect();
        let locate = |key: &str| {
            let (id, control) = crate::config::source(key)?;
            Some((devices.iter().position(|d| d.id == id)?, control))
        };
        for (key, targets) in &profile.buttons {
            let mask = targets.iter().fold(0u16, |mask, target| mask | 1 << (target - 1));
            match locate(key) {
                Some((device, Control::Button(index))) if index < slots[device].targets.len() => slots[device].targets[index] |= mask,
                Some((device, Control::Hat(index))) => slots[device].hat_targets[index] |= mask,
                _ => {}
            }
        }
        for (key, direction) in &profile.pov {
            match locate(key) {
                Some((device, Control::Button(index))) if index < slots[device].pov.len() => slots[device].pov[index] = Some(*direction),
                Some((device, Control::Hat(index))) => slots[device].hat_pov[index] = Some(*direction),
                _ => {}
            }
        }
        let hatted = |id: &str| devices.iter().position(|d| d.id == id && d.layout.hat);
        let camera = profile.camera.as_deref().and_then(hatted).or_else(|| devices.iter().position(|d| d.layout.hat));
        let axes = &profile.axes;
        let bind = |binding: &Option<Binding>| binding.as_ref().and_then(|b| resolve(devices, b));
        let levers: Vec<Bound> = profile.split_high_g.levers.iter().filter_map(|b| resolve(devices, b)).collect();
        Self {
            deadzone: config.deadzone,
            strength: config.camera_strength,
            hat: profile.hat,
            camera,
            slots,
            roll: bind(&axes.roll),
            pitch: bind(&axes.pitch),
            yaw: bind(&axes.yaw),
            throttle: axes.throttle.iter().filter_map(|b| resolve(devices, b)).collect(),
            rule: profile.split_high_g.clone(),
            levers: <[Bound; 2]>::try_from(levers).ok(),
            selector: ThrottleSelector::default(),
        }
    }

    fn flight_axis(&self, bound: Option<Bound>, inputs: &[&InputState]) -> u16 {
        let Some(bound) = bound else { return 32768 };
        let (raw, maximum) = (u64::from(inputs[bound.device].axes[bound.axis].min(bound.max)), u64::from(bound.max));
        if self.deadzone == 0.0 {
            let result = (raw * 65535 / maximum) as u16;
            return if bound.invert { 65535 - result } else { result };
        }
        let value = centered(2.0 * raw as f64 / maximum as f64 - 1.0, self.deadzone);
        unit16(if bound.invert { -value } else { value })
    }

    /// Lever travel of the split rule's two levers (0 idle, 1 fully forward), when both are bound and reporting.
    pub fn levers(&self, inputs: &[Option<InputState>]) -> Option<[f64; 2]> {
        let levers = self.levers?;
        let travel = |b: Bound| {
            let state = inputs.get(b.device)?.as_ref()?;
            let fraction = f64::from(state.axes[b.axis]) / f64::from(b.max);
            Some(if self.rule.reverse { fraction } else { 1.0 - fraction })
        };
        Some([travel(levers[0])?, travel(levers[1])?])
    }

    /// Missing input from any device resets throttle ownership and yields the neutral report.
    pub fn convert(&mut self, inputs: &[Option<InputState>]) -> Report {
        let present: Option<Vec<&InputState>> = inputs.iter().map(Option::as_ref).collect();
        let Some(states) = present.filter(|s| s.len() == self.slots.len() && !s.is_empty()) else {
            self.selector = ThrottleSelector::default();
            return neutral();
        };
        let values: Vec<f64> = self.throttle.iter().map(|b| 2.0 * f64::from(states[b.device].axes[b.axis].min(b.max)) / f64::from(b.max) - 1.0).collect();
        let active = self.selector.read(&values);
        let positions = self.levers(inputs);
        // Split High-G: the throttle follows the forward lever (the greater travel), converted back to the raw axis
        // convention with that lever's inversion; NEO 5 is held below.
        let split = positions.is_some_and(|p| split(&self.rule, p));
        let throttle = match (positions.filter(|_| split), active) {
            (Some(p), _) => {
                let lever = self.levers.unwrap()[usize::from(p[1] > p[0])];
                let travel = p[0].max(p[1]);
                let value = if self.rule.reverse { 2.0 * travel - 1.0 } else { 1.0 - 2.0 * travel };
                unit16(if lever.invert { -value } else { value })
            }
            (None, Some((index, value))) => unit16(if self.throttle[index].invert { -value } else { value }),
            (None, None) => 32768,
        };
        // Camera mode: the camera hat moves the mini-stick. Other hats, and every hat in mapped mode, act as sources.
        let camera = self.camera.filter(|_| self.hat == HatMode::Camera);
        let view = camera.and_then(|c| states[c].hat).map_or((0, 0), |h| HATS[usize::from(h)]);
        let axes = [
            self.flight_axis(self.roll, &states),
            self.flight_axis(self.pitch, &states),
            throttle,
            unit16(f64::from(view.0) * self.strength),
            unit16(f64::from(view.1) * self.strength),
            self.flight_axis(self.yaw, &states),
            32768,
        ];
        let mut buttons = u16::from(split) << 4;
        let mut order = (0i8, 0i8);
        for (index, (slot, state)) in self.slots.iter().zip(&states).enumerate() {
            let hat = state.hat.filter(|_| camera != Some(index)).map_or((0, 0), |h| HATS[usize::from(h)]);
            let held = [hat.1 < 0, hat.0 > 0, hat.1 > 0, hat.0 < 0];
            let sources = (0..slot.targets.len())
                .filter(|&i| state.buttons & 1 << i != 0)
                .map(|i| (slot.targets[i], slot.pov[i]))
                .chain((0..4).filter(|&i| held[i]).map(|i| (slot.hat_targets[i], slot.hat_pov[i])));
            for (targets, direction) in sources {
                buttons |= targets;
                let (x, y) = direction.map_or((0, 0), Direction::vector);
                order = (order.0 + x, order.1 + y);
            }
        }
        let order = (order.0.clamp(-1, 1), order.1.clamp(-1, 1));
        let hat = HATS.iter().position(|&h| h == order).map_or(8, |p| p as u8);
        input_report(axes, buttons, hat)
    }
}
