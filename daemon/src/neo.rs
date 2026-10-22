//! NEO report 01 encoding and the controller-to-NEO mapping (port of tca_to_neo.py / contract_probe.py,
//! generalised from the TCA pair to any set of joysticks).
use crate::config::{AxisTuning, Binding, Config, Control, Direction, HatMode, Mode, Profile, SplitHighG, Target, Throttle, ThrottleMode, YawSource};
use crate::hid::InputState;
use crate::models::DeviceInfo;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;

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

/// One lever at or above the forward threshold and the other at or below the back threshold, in either order.
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

/// Detent throttle: throttle travel (0 idle, 1 forward) to -1 full brake, 0 cruise, +1 full boost.
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
    NextProfile,
}

/// Button combos and the event each raises when completed.
const COMBO_EVENTS: [Event; 2] = [Event::TogglePause, Event::NextProfile];

/// Raw inputs next to what the NEO receives, for the page's live previews. Flight inputs are -1..1 as bound (before
/// inversion); throttle travel and the split levers run from 0 at idle to 1 fully forward.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Monitor {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    /// Travel of the throttle input in control (the forward lever while split); None without a throttle axis.
    pub throttle: Option<f64>,
    /// The camera lever's position, -1..1 after its inversion.
    pub camera_lever: Option<f64>,
    pub levers: Option<[f64; 2]>,
    pub axes: [u16; 7],
    pub armed: bool,
    pub precision: bool,
    /// The shift control is held.
    pub shift: bool,
    /// NEO buttons 1..15 as bits 0..14, and the POV (8 when centred), as sent.
    pub buttons: u16,
    pub pov: u8,
    /// The head-tracking sample (degrees) the camera used this frame.
    pub head: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Copy, Default)]
struct SourceState {
    down: bool,
    /// The press (or tap) output is on until this time.
    until: Duration,
    latched: bool,
    next_repeat: Duration,
    pressed_at: Duration,
    /// The long-press targets are held.
    long: bool,
    /// This press was the second tap of a double tap: its release sends nothing.
    consumed: bool,
    /// A tap was released at this time and may still become a double tap.
    waiting: Option<Duration>,
    double_until: Duration,
}

/// NEO buttons mask and POV direction a source sends.
type Mapping = (u16, Option<Direction>);

fn mapping(target: &Target) -> Mapping {
    (mask(&target.buttons), target.pov)
}

fn mask(targets: &[u8]) -> u16 {
    targets.iter().fold(0u16, |mask, target| mask | 1 << (target - 1))
}

/// A camera lever pans the camera around its mid travel, ignoring small offsets.
const CAMERA_LEVER: AxisTuning = AxisTuning { deadzone: 0.05, saturation: 1.0, anti_deadzone: 0.0, curve: 0.0, smoothing: 0.0 };
const IDENTITY: AxisTuning = AxisTuning { deadzone: 0.0, saturation: 1.0, anti_deadzone: 0.0, curve: 0.0, smoothing: 0.0 };

#[derive(Debug, Clone, Copy)]
struct Bound {
    device: usize,
    axis: usize,
    max: u32,
    invert: bool,
}

impl Bound {
    /// -1 at the low raw end to 1 at the high end, before inversion.
    fn unit(self, states: &[&InputState]) -> f64 {
        2.0 * f64::from(states[self.device].axes[self.axis].min(self.max)) / f64::from(self.max) - 1.0
    }

    fn signed(self, states: &[&InputState]) -> f64 {
        if self.invert {
            -self.unit(states)
        } else {
            self.unit(states)
        }
    }
}

fn resolve(devices: &[DeviceInfo], binding: &Binding) -> Option<Bound> {
    let device = devices.iter().position(|d| d.id == binding.device)?;
    let axis = devices[device].layout.axis(&binding.axis)?;
    Some(Bound { device, axis, max: devices[device].layout.axes[axis].max.max(1), invert: binding.invert })
}

/// Mappable sources in one index space: each device's buttons, then its four hat directions.
fn offsets(devices: &[DeviceInfo]) -> Vec<usize> {
    devices.iter().scan(0, |next, d| Some(std::mem::replace(next, *next + usize::from(d.layout.buttons) + 4))).collect()
}

#[derive(Debug, Clone)]
pub struct Mapper {
    config: Config,
    profile: Profile,
    devices: Vec<DeviceInfo>,
    offsets: Vec<usize>,
    mappings: Vec<Mapping>,
    shifted: Vec<Option<Mapping>>,
    long: Vec<Option<Mapping>>,
    double: Vec<Option<Mapping>>,
    shift: Option<usize>,
    modes: Vec<Mode>,
    states: Vec<SourceState>,
    /// False until the first complete frame: sources already down then (a latched switch) are not edges.
    primed: bool,
    precision: Option<usize>,
    /// Source slots of the pause and next-profile combos (empty: off), and whether each was complete last frame.
    combos: [Vec<usize>; 2],
    combos_held: [bool; 2],
    camera_hat: Option<usize>,
    roll: Option<Bound>,
    pitch: Option<Bound>,
    yaw: Option<Bound>,
    throttle: Vec<Bound>,
    levers: Option<[Bound; 2]>,
    camera_lever: Option<Bound>,
    selector: ThrottleSelector,
    armed: bool,
    start_side: Option<bool>,
    smoothed: [Option<f64>; 3],
    /// When the hat started moving the camera, for the ramp and full strength after a long press.
    hat_since: Option<Duration>,
    /// Head yaw and pitch in degrees from head tracking, when a fresh sample exists.
    head: Option<[f64; 2]>,
    now: Duration,
    events: Vec<Event>,
    monitor: Option<Monitor>,
}

impl Mapper {
    /// `devices` are the session's devices in input order. The config must already be validated; bindings and
    /// sources naming absent devices, axes or buttons are ignored.
    pub fn new(config: &Config, devices: &[DeviceInfo]) -> Self {
        Self::with_profile(config, &config.profile(devices), devices)
    }

    pub fn with_profile(config: &Config, profile: &Profile, devices: &[DeviceInfo]) -> Self {
        let offsets = offsets(devices);
        let count = devices.iter().map(|d| usize::from(d.layout.buttons) + 4).sum();
        let slot = |key: &str| {
            let (id, control) = crate::config::source(key)?;
            let device = devices.iter().position(|d| d.id == id)?;
            match control {
                Control::Button(index) => (index < usize::from(devices[device].layout.buttons)).then_some(offsets[device] + index),
                Control::Hat(index) => devices[device].layout.hat.then_some(offsets[device] + usize::from(devices[device].layout.buttons) + index),
            }
        };
        let mut mappings = vec![(0, None); count];
        let mut shifted = vec![None; count];
        let mut long = vec![None; count];
        let mut double = vec![None; count];
        let mut modes = vec![Mode::Hold; count];
        for (key, targets) in &profile.buttons {
            slot(key).into_iter().for_each(|s| mappings[s].0 = mask(targets));
        }
        for (key, direction) in &profile.pov {
            slot(key).into_iter().for_each(|s| mappings[s].1 = Some(*direction));
        }
        // A shifted source without a shifted POV (or buttons) sends nothing of that kind in the shift layer.
        for (key, targets) in &profile.shifted.buttons {
            slot(key).into_iter().for_each(|s| shifted[s].get_or_insert((0, None)).0 = mask(targets));
        }
        for (key, direction) in &profile.shifted.pov {
            slot(key).into_iter().for_each(|s| shifted[s].get_or_insert((0, None)).1 = Some(*direction));
        }
        for (key, target) in &profile.long_press {
            slot(key).into_iter().for_each(|s| long[s] = Some(mapping(target)));
        }
        for (key, target) in &profile.double_tap {
            slot(key).into_iter().for_each(|s| double[s] = Some(mapping(target)));
        }
        for (key, mode) in &profile.behaviours {
            slot(key).into_iter().for_each(|s| modes[s] = *mode);
        }
        // A combo naming a control that is not here can never complete: it is off.
        let combo = |keys: &[String]| keys.iter().map(|k| slot(k)).collect::<Option<Vec<usize>>>().unwrap_or_default();
        let hatted = |id: &str| devices.iter().position(|d| d.id == id && d.layout.hat);
        let bind = |binding: &Option<Binding>| binding.as_ref().and_then(|b| resolve(devices, b));
        let levers: Vec<Bound> = profile.split_high_g.levers.iter().filter_map(|b| resolve(devices, b)).collect();
        let bindings = &profile.bindings;
        Self {
            config: config.clone(),
            profile: profile.clone(),
            devices: devices.to_vec(),
            offsets: offsets.clone(),
            mappings,
            shifted,
            long,
            double,
            shift: profile.shift.as_deref().and_then(slot),
            modes,
            states: vec![SourceState::default(); count],
            primed: false,
            precision: profile.precision.source.as_deref().and_then(slot),
            combos: [combo(&profile.combos.pause), combo(&profile.combos.next_profile)],
            combos_held: [false; 2],
            camera_hat: profile.camera_hat.as_deref().and_then(hatted).or_else(|| devices.iter().position(|d| d.layout.hat)),
            roll: bind(&bindings.roll),
            pitch: bind(&bindings.pitch),
            yaw: bind(&bindings.yaw),
            throttle: bindings.throttle.iter().filter_map(|b| resolve(devices, b)).collect(),
            levers: <[Bound; 2]>::try_from(levers).ok(),
            camera_lever: bind(&profile.camera_lever),
            selector: ThrottleSelector::default(),
            armed: !config.safe_start,
            start_side: None,
            smoothed: [None; 3],
            hat_since: None,
            head: None,
            now: Duration::ZERO,
            events: Vec::new(),
            monitor: None,
        }
    }

    /// New settings mid-flight: keep throttle ownership, safe-start arming, smoothing and source states, so saving
    /// or previewing never fires a latched switch or drops the throttle.
    pub fn reconfigure(&mut self, config: &Config) {
        let mut next = Mapper::new(config, &self.devices);
        next.states = std::mem::take(&mut self.states);
        next.primed = self.primed;
        next.combos_held = self.combos_held;
        next.selector = self.selector.clone();
        next.armed |= self.armed;
        next.start_side = self.start_side;
        next.smoothed = self.smoothed;
        next.hat_since = self.hat_since;
        next.head = self.head;
        next.now = self.now;
        *self = next;
    }

    /// The head-tracking sample for the next frame (None: no fresh data).
    pub fn head(&mut self, sample: Option<[f64; 2]>) -> &mut Self {
        self.head = sample;
        self
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

    fn tuning(&self, axis: usize) -> AxisTuning {
        [self.config.axes.roll, self.config.axes.pitch, self.config.axes.yaw][axis]
    }

    /// A bound flight axis through its tuning and the precision scale; untuned, unscaled axes pass bit-exact.
    fn flight_axis(&mut self, axis: usize, bound: Option<Bound>, states: &[&InputState], scale: f64) -> u16 {
        let Some(bound) = bound else { return 32768 };
        if self.tuning(axis) == IDENTITY && scale == 1.0 {
            let (raw, maximum) = (u64::from(states[bound.device].axes[bound.axis].min(bound.max)), u64::from(bound.max));
            let result = (raw * 65535 / maximum) as u16;
            return if bound.invert { 65535 - result } else { result };
        }
        self.shaped_axis(axis, bound.unit(states), bound.invert, scale)
    }

    /// One axis value in -1..1 through its tuning, precision scale, smoothing and inversion.
    fn shaped_axis(&mut self, axis: usize, value: f64, invert: bool, scale: f64) -> u16 {
        let tuning = self.tuning(axis);
        let value = shape(value, &tuning) * scale;
        let value = self.smoothed[axis].map_or(value, |previous| previous * tuning.smoothing + value * (1.0 - tuning.smoothing));
        self.smoothed[axis] = Some(value);
        unit16(if invert { -value } else { value })
    }

    /// Whether a source drives its press, long-press and double-tap targets this frame. With a long press or double
    /// tap set, the press is resolved on release (and after the double-tap window) and sent as a pulse.
    fn drive(&mut self, slot: usize, down: bool) -> [bool; 3] {
        let timing = self.config.timing;
        let ms = |millis: u32| Duration::from_millis(u64::from(millis));
        let (now, pulse) = (self.now, ms(timing.pulse_ms));
        let (long, double) = (self.long[slot].is_some(), self.double[slot].is_some());
        let mode = self.modes[slot];
        let state = &mut self.states[slot];
        let (rising, changed) = (down && !state.down, down != state.down);
        state.down = down;
        if long || double {
            if rising && state.waiting.take().is_some() {
                state.double_until = now + pulse;
                state.consumed = true;
            } else if rising {
                state.pressed_at = now;
            }
            state.long |= down && long && !state.consumed && now >= state.pressed_at + ms(timing.long_ms);
            if changed && !down {
                if state.long || state.consumed {
                    (state.long, state.consumed) = (false, false);
                } else if double {
                    state.waiting = Some(now);
                } else {
                    state.until = now + pulse;
                }
            }
            if state.waiting.is_some_and(|since| now >= since + ms(timing.double_ms)) {
                state.waiting = None;
                state.until = now + pulse;
            }
            return [now < state.until, state.long, now < state.double_until];
        }
        let press = match mode {
            Mode::Hold => down,
            Mode::Toggle => {
                state.latched ^= rising;
                state.latched
            }
            Mode::Repeat => {
                if rising || (down && now >= state.next_repeat) {
                    (state.until, state.next_repeat) = (now + pulse, now + ms(timing.repeat_ms));
                }
                now < state.until
            }
            mode => {
                if rising || (mode == Mode::PulseChange && changed) {
                    state.until = now + pulse;
                }
                now < state.until
            }
        };
        [press, false, false]
    }

    /// Camera deflection from the hat: ramped in over `camera.ramp_ms`, full after a long press if set.
    fn camera(&mut self, hat: (i8, i8)) -> f64 {
        if hat == (0, 0) {
            self.hat_since = None;
            return 0.0;
        }
        let camera = self.config.camera;
        let held = self.now.saturating_sub(*self.hat_since.get_or_insert(self.now));
        if camera.full_after_hold && held >= Duration::from_millis(u64::from(self.config.timing.long_ms)) {
            return 1.0;
        }
        let ramp = if camera.ramp_ms == 0 { 1.0 } else { (held.as_secs_f64() * 1000.0 / f64::from(camera.ramp_ms)).min(1.0) };
        self.config.camera_strength * ramp
    }

    /// The NEO throttle (-1 forward … +1 idle, the raw-axis convention) for a throttle input in that convention.
    fn throttle_value(&self, value: f64) -> f64 {
        match self.config.throttle.mode {
            ThrottleMode::Linear => value,
            ThrottleMode::Detents => -detent((1.0 - value) / 2.0, &self.config.throttle),
        }
    }

    /// Lever travel of the split rule's two levers (0 idle, 1 fully forward), when both are bound and reporting.
    pub fn levers(&self, inputs: &[Option<InputState>]) -> Option<[f64; 2]> {
        let levers = self.levers?;
        let travel = |b: Bound| {
            let state = inputs.get(b.device)?.as_ref()?;
            let fraction = f64::from(state.axes[b.axis].min(b.max)) / f64::from(b.max);
            Some(if self.profile.split_high_g.reverse { fraction } else { 1.0 - fraction })
        };
        Some([travel(levers[0])?, travel(levers[1])?])
    }

    /// Pressed state of every source slot. In camera mode the camera hat moves the mini-stick and is no source.
    fn sources(&self, states: &[&InputState], camera: Option<usize>) -> Vec<bool> {
        let mut pressed = vec![false; self.states.len()];
        for (index, (device, state)) in self.devices.iter().zip(states).enumerate() {
            let base = self.offsets[index];
            let buttons = usize::from(device.layout.buttons);
            (0..buttons).for_each(|i| pressed[base + i] = state.buttons & 1 << i != 0);
            if device.layout.hat {
                let hat = state.hat.filter(|_| camera != Some(index)).map_or((0, 0), |h| HATS[usize::from(h)]);
                let held = [hat.1 < 0, hat.0 > 0, hat.1 > 0, hat.0 < 0];
                (0..4).for_each(|i| pressed[base + buttons + i] = held[i]);
            }
        }
        pressed
    }

    /// Missing input from any device resets throttle ownership, disarms a safe start and yields the neutral report.
    pub fn convert(&mut self, inputs: &[Option<InputState>]) -> Report {
        let present: Option<Vec<&InputState>> = inputs.iter().map(Option::as_ref).collect();
        let Some(states) = present.filter(|s| s.len() == self.devices.len() && !s.is_empty()) else {
            self.selector = ThrottleSelector::default();
            self.armed = !self.config.safe_start;
            self.start_side = None;
            self.primed = false;
            self.monitor = None;
            return neutral();
        };
        let camera = self.camera_hat.filter(|_| self.profile.hat == HatMode::Camera);
        let pressed = self.sources(&states, camera);
        if !self.primed {
            for (state, &down) in self.states.iter_mut().zip(&pressed) {
                state.down = down;
            }
            self.combos_held = self.combos.clone().map(|slots| !slots.is_empty() && slots.iter().all(|&s| pressed[s]));
            self.primed = true;
        }
        for (index, slots) in self.combos.clone().into_iter().enumerate() {
            let complete = !slots.is_empty() && slots.iter().all(|&s| pressed[s]);
            if complete && !self.combos_held[index] {
                self.events.push(COMBO_EVENTS[index]);
            }
            self.combos_held[index] = complete;
        }
        let precise = self.precision.is_some_and(|slot| pressed[slot]);
        let scale = if precise { self.profile.precision.scale } else { 1.0 };

        // Throttle: the last throttle axis moved owns it; split High-G hands it to the forward lever and holds NEO 5.
        let values: Vec<f64> = self.throttle.iter().map(|b| b.unit(&states)).collect();
        let active = self.selector.read(&values);
        let positions = self.levers(inputs);
        let split = positions.is_some_and(|p| split(&self.profile.split_high_g, p));
        // Lever yaw uses both levers, so the throttle follows their average.
        let differential = positions.filter(|_| self.profile.yaw_source == YawSource::LeverDifferential);
        let lever_input = |lever: Bound, travel: f64| {
            let raw = if self.profile.split_high_g.reverse { 2.0 * travel - 1.0 } else { 1.0 - 2.0 * travel };
            if lever.invert {
                -raw
            } else {
                raw
            }
        };
        let input = match (positions.filter(|_| split), differential, active) {
            (Some(p), _, _) => Some(lever_input(self.levers.unwrap()[usize::from(p[1] > p[0])], p[0].max(p[1]))),
            (None, Some(p), _) => Some(lever_input(self.levers.unwrap()[0], (p[0] + p[1]) / 2.0)),
            (None, None, Some((index, value))) => Some(if self.throttle[index].invert { -value } else { value }),
            (None, None, None) => None,
        };
        let value = input.map_or(0.0, |v| self.throttle_value(v));
        // Safe start: the throttle rests at centre until the throttle in control reaches it or moves across it.
        if !self.armed {
            let side = value > 0.0;
            let crossed = *self.start_side.get_or_insert(side) != side;
            self.armed = crossed || value.abs() <= ARM_BAND;
        }
        let throttle = if self.armed { unit16(value) } else { 32768 };

        // Camera sources by priority: the hat, then head tracking, then the camera lever.
        let view = camera.and_then(|c| states[c].hat).map_or((0, 0), |h| HATS[usize::from(h)]);
        let deflection = self.camera(view);
        let tracking = self.config.head_tracking;
        let head = self.head.filter(|_| view == (0, 0) && tracking.enabled);
        let lever = self.camera_lever.map(|b| b.signed(&states));
        let (camera_x, camera_y) = if let Some([yaw, pitch]) = head {
            let sign = |invert: bool| if invert { -1.0 } else { 1.0 };
            // Looking up (positive pitch) is camera up, which the NEO reports as negative Ry.
            (shape(sign(tracking.invert_yaw) * yaw / tracking.yaw_range, &tracking.tuning), shape(-sign(tracking.invert_pitch) * pitch / tracking.pitch_range, &tracking.tuning))
        } else if let Some(position) = lever.filter(|_| view == (0, 0)) {
            (shape(position, &CAMERA_LEVER) * self.config.camera_strength, 0.0)
        } else {
            (f64::from(view.0) * deflection, f64::from(view.1) * deflection)
        };

        let (yaw, yaw_input) = match self.profile.yaw_source {
            YawSource::Twist => (self.flight_axis(2, self.yaw, &states, scale), self.yaw.map_or(0.0, |b| b.unit(&states))),
            YawSource::Off => (32768, 0.0),
            YawSource::LeverDifferential => match positions {
                Some(p) => (self.shaped_axis(2, p[0] - p[1], self.yaw.is_some_and(|b| b.invert), scale), p[0] - p[1]),
                None => (32768, 0.0),
            },
        };
        let axes = [
            self.flight_axis(0, self.roll, &states, scale),
            self.flight_axis(1, self.pitch, &states, scale),
            throttle,
            unit16(camera_x),
            unit16(camera_y),
            yaw,
            32768,
        ];
        let mut buttons = u16::from(split) << 4;
        let mut order = (0i8, 0i8);
        let shifted = self.shift.is_some_and(|slot| pressed[slot]);
        for (slot, &down) in pressed.iter().enumerate() {
            let [press, long, double] = self.drive(slot, down);
            if self.shift == Some(slot) {
                continue; // the shift control sends nothing; its state is still tracked for when it stops being one
            }
            let base = if shifted { self.shifted[slot].unwrap_or(self.mappings[slot]) } else { self.mappings[slot] };
            for (targets, direction) in [(press, Some(base)), (long, self.long[slot]), (double, self.double[slot])].into_iter().filter_map(|(on, m)| m.filter(|_| on)) {
                buttons |= targets;
                let (x, y) = direction.map_or((0, 0), Direction::vector);
                order = (order.0 + x, order.1 + y);
            }
        }
        let order = (order.0.clamp(-1, 1), order.1.clamp(-1, 1));
        let hat = HATS.iter().position(|&h| h == order).map_or(8, |p| p as u8);
        let raw = |bound: Option<Bound>| bound.map_or(0.0, |b| b.unit(&states));
        self.monitor = Some(Monitor {
            roll: raw(self.roll),
            pitch: raw(self.pitch),
            yaw: yaw_input,
            throttle: input.map(|v| (1.0 - v) / 2.0),
            camera_lever: lever,
            levers: positions,
            axes,
            armed: self.armed,
            precision: precise,
            shift: shifted,
            buttons,
            pov: hat,
            head,
        });
        input_report(axes, buttons, hat)
    }
}
