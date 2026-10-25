//! NEO report 01 encoding and the TCA-to-NEO mapping (port of tca_to_neo.py / contract_probe.py).
use crate::config::{AxisTuning, Config, Device, Direction, HatMode, Lever, Mode, SplitHighG, Target, ThrottleMode, YawSource};
pub use neo_common::api::Monitor;
pub use neo_common::maths::{centered, detent, shape, unit16, ARM_BAND, HAT_NAMES};
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

/// Side effects the emulation loop applies; the report itself stays a pure function of the inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    TogglePause,
    NextProfile,
}

/// Button combos and the event each raises when completed.
const COMBO_EVENTS: [Event; 2] = [Event::TogglePause, Event::NextProfile];

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

/// The spare quadrant lever pans the camera around its mid travel, ignoring small offsets.
const CAMERA_LEVER: AxisTuning = AxisTuning { deadzone: 0.05, saturation: 1.0, anti_deadzone: 0.0, curve: 0.0, smoothing: 0.0 };

#[derive(Debug, Clone)]
pub struct Mapper {
    config: Config,
    mappings: [Mapping; SOURCES],
    shifted: [Option<Mapping>; SOURCES],
    long: [Option<Mapping>; SOURCES],
    double: [Option<Mapping>; SOURCES],
    shift: Option<usize>,
    modes: [Mode; SOURCES],
    states: [SourceState; SOURCES],
    /// False until the first complete frame: sources already down then (a latched switch) are not edges.
    primed: bool,
    precision: Option<usize>,
    /// Source masks of the pause and next-profile combos (0: off), and whether each was complete last frame.
    combos: [u64; 2],
    combos_held: [bool; 2],
    throttle: ThrottleSelector,
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
    /// The config must already be validated (config::validate).
    pub fn new(config: &Config) -> Self {
        let mut mapper = Self {
            config: config.clone(),
            mappings: [(0, None); SOURCES],
            shifted: [None; SOURCES],
            long: [None; SOURCES],
            double: [None; SOURCES],
            shift: config.shift.as_deref().map(slot),
            modes: [Mode::Hold; SOURCES],
            states: [SourceState::default(); SOURCES],
            primed: false,
            precision: config.precision.source.as_deref().map(slot),
            combos: [&config.combos.pause, &config.combos.next_profile].map(|keys| keys.iter().fold(0, |mask, key| mask | 1 << slot(key))),
            combos_held: [false; 2],
            throttle: ThrottleSelector::default(),
            armed: !config.safe_start,
            start_side: None,
            smoothed: [None; 3],
            hat_since: None,
            head: None,
            now: Duration::ZERO,
            events: Vec::new(),
            monitor: None,
        };
        for (key, targets) in &config.buttons {
            mapper.mappings[slot(key)].0 = mask(targets);
        }
        for (key, direction) in &config.pov {
            mapper.mappings[slot(key)].1 = Some(*direction);
        }
        // A shifted source without a shifted POV (or buttons) sends nothing of that kind in the shift layer.
        for (key, targets) in &config.shifted.buttons {
            mapper.shifted[slot(key)].get_or_insert((0, None)).0 = mask(targets);
        }
        for (key, direction) in &config.shifted.pov {
            mapper.shifted[slot(key)].get_or_insert((0, None)).1 = Some(*direction);
        }
        for (key, target) in &config.long_press {
            mapper.long[slot(key)] = Some(mapping(target));
        }
        for (key, target) in &config.double_tap {
            mapper.double[slot(key)] = Some(mapping(target));
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
        next.combos_held = self.combos_held;
        next.throttle = self.throttle.clone();
        next.armed |= self.armed;
        next.start_side = self.start_side;
        next.smoothed = self.smoothed;
        next.hat_since = self.hat_since;
        next.head = self.head;
        next.now = self.now;
        *self = next;
    }

    /// Time of the next frame, for pulses (any monotonic clock).
    /// The head-tracking sample for the next frame (None: no fresh data).
    pub fn head(&mut self, sample: Option<[f64; 2]>) -> &mut Self {
        self.head = sample;
        self
    }

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

    fn flight_axis(&mut self, axis: usize, raw: u32, maximum: u32, invert: bool, scale: f64) -> u16 {
        let raw = raw.min(maximum);
        if self.tuning(axis) == (AxisTuning { deadzone: 0.0, saturation: 1.0, anti_deadzone: 0.0, curve: 0.0, smoothing: 0.0 }) && scale == 1.0 {
            let result = (raw * 65535 / maximum) as u16;
            return if invert { 65535 - result } else { result };
        }
        self.shaped_axis(axis, 2.0 * f64::from(raw) / f64::from(maximum) - 1.0, invert, scale)
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
            self.combos_held = self.combos.map(|mask| mask != 0 && pressed & mask == mask);
            self.primed = true;
        }
        for (index, mask) in self.combos.into_iter().enumerate() {
            let complete = mask != 0 && pressed & mask == mask;
            if complete && !self.combos_held[index] {
                self.events.push(COMBO_EVENTS[index]);
            }
            self.combos_held[index] = complete;
        }
        let precise = self.precision.is_some_and(|slot| pressed >> slot & 1 == 1);
        let scale = if precise { self.config.precision.scale } else { 1.0 };

        let roll = u32::from(u16::from_le_bytes([stick[1], stick[2]]));
        let pitch = u32::from(u16::from_le_bytes([stick[3], stick[4]]));
        let lever_index = match self.config.throttle_lever {
            Lever::Left => 0,
            Lever::Right => 1,
        };
        let positions = levers(quadrant);
        // Lever yaw uses both levers, so the throttle follows their average.
        let differential = self.config.yaw_source == YawSource::LeverDifferential;
        let raw_lever = |index: usize| u32::from(u16::from_le_bytes([quadrant[3 + 2 * index], quadrant[4 + 2 * index]]));
        let lever = if differential { (raw_lever(0) + raw_lever(1)) / 2 } else { raw_lever(lever_index) };
        let travel = if differential { (positions[0] + positions[1]) / 2.0 } else { positions[lever_index] };
        let active = self.throttle.read(2.0 * f64::from(stick[6]) / 255.0 - 1.0, 2.0 * f64::from(lever) / 65535.0 - 1.0);
        // Split High-G: the throttle follows the forward lever (the greater travel), with the same response and
        // inversion as the normal throttle; NEO 5 is held below.
        let split = split(&self.config.split_high_g, positions);
        let value = if split {
            self.lever_throttle(positions[0].max(positions[1]))
        } else if self.throttle.active() == 1 && self.config.throttle.mode == ThrottleMode::Detents {
            self.lever_throttle(travel)
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
        // The hat wins over the spare-lever camera while it is pressed.
        let deflection = self.camera(camera);
        // Camera sources by priority: the hat, then head tracking, then the spare lever.
        let tracking = &self.config.head_tracking;
        let head = self.head.filter(|_| camera == (0, 0) && tracking.enabled);
        let (camera_x, camera_y) = if let Some([yaw, pitch]) = head {
            let sign = |invert: bool| if invert { -1.0 } else { 1.0 };
            // Looking up (positive pitch) is camera up, which the NEO reports as negative Ry.
            (shape(sign(tracking.invert_yaw) * yaw / tracking.yaw_range, &tracking.tuning), shape(-sign(tracking.invert_pitch) * pitch / tracking.pitch_range, &tracking.tuning))
        } else if camera == (0, 0) && self.config.camera.lever {
            (shape(2.0 * positions[1 - lever_index] - 1.0, &CAMERA_LEVER) * self.config.camera_strength, 0.0)
        } else {
            (f64::from(camera.0) * deflection, f64::from(camera.1) * deflection)
        };
        let yaw = match self.config.yaw_source {
            YawSource::Twist => self.flight_axis(2, u32::from(stick[5]), 255, invert.yaw, scale),
            YawSource::Off => 32768,
            YawSource::LeverDifferential => self.shaped_axis(2, positions[0] - positions[1], invert.yaw, scale),
        };
        let axes = [
            self.flight_axis(0, roll, 16383, invert.roll, scale),
            self.flight_axis(1, pitch, 16383, invert.pitch, scale),
            throttle,
            unit16(camera_x),
            unit16(camera_y),
            yaw,
            32768,
        ];
        let mut buttons = u16::from(split) << 4;
        let mut order = (0i8, 0i8);
        let shifted = self.shift.is_some_and(|slot| pressed >> slot & 1 == 1);
        for slot in 0..SOURCES {
            let [press, long, double] = self.drive(slot, pressed >> slot & 1 == 1);
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
            shift: shifted,
            buttons,
            pov: hat,
            head,
        });
        input_report(axes, buttons, hat)
    }
}
