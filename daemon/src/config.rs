//! Private NEO mapping profile: validation, defaults and atomic persistence.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Stick,
    Quadrant,
    Hat,
}

/// Mappable sidestick hat directions, in "hat-<direction>" source keys; diagonals press two of them.
pub const HAT_DIRECTIONS: [&str; 4] = ["up", "right", "down", "left"];

/// What the sidestick hat drives: the NEO camera mini-stick (research default) or its own mappings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HatMode {
    #[default]
    Camera,
    Mapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lever {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn vector(self) -> (i8, i8) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }
}

/// Hold NEO 5 (High-G) while one quadrant lever is at or above `forward` and the other at or below `back`
/// (fractions of lever travel, either order); the throttle follows the forward lever meanwhile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitHighG {
    pub enabled: bool,
    pub forward: f64,
    pub back: f64,
}

impl Default for SplitHighG {
    fn default() -> Self {
        Self { enabled: true, forward: 0.75, back: 0.25 }
    }
}

/// Response shaping for one flight axis, applied in this order: deadzone, saturation (stick travel that already
/// gives full output), curve (positive softens the centre, negative sharpens it), anti-deadzone (smallest output
/// once past the deadzone), then smoothing (the share of the previous output kept each frame).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisTuning {
    pub deadzone: f64,
    pub saturation: f64,
    pub anti_deadzone: f64,
    pub curve: f64,
    pub smoothing: f64,
}

impl Default for AxisTuning {
    fn default() -> Self {
        Self { deadzone: 0.04, saturation: 1.0, anti_deadzone: 0.0, curve: 0.0, smoothing: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axes {
    pub roll: AxisTuning,
    pub pitch: AxisTuning,
    pub yaw: AxisTuning,
}

/// Linear: lever travel maps straight to the NEO throttle. Detents: ACE COMBAT's centre-cruise model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThrottleMode {
    #[default]
    Linear,
    Detents,
}

/// Detent bands in quadrant lever travel (0 idle, 1 fully forward): full brake up to `idle`, cruise within `band`
/// of `cruise`, full boost from `boost`, linear in between.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Throttle {
    pub mode: ThrottleMode,
    pub idle: f64,
    pub cruise: f64,
    pub boost: f64,
    pub band: f64,
}

impl Default for Throttle {
    fn default() -> Self {
        Self { mode: ThrottleMode::Linear, idle: 0.10, cruise: 0.50, boost: 0.90, band: 0.03 }
    }
}

/// While `source` is held, roll, pitch and yaw are multiplied by `scale`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Precision {
    pub source: Option<String>,
    pub scale: f64,
}

impl Default for Precision {
    fn default() -> Self {
        Self { source: None, scale: 0.5 }
    }
}

/// Sources pressed together; an empty combo is off.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Combos {
    pub pause: Vec<String>,
    /// Switches to the next saved profile, in name order.
    #[serde(default)]
    pub next_profile: Vec<String>,
}

/// How a source drives its NEO targets: held while pressed, a short press on switch-on, a short press on every
/// change, flipped on each press, or short presses repeated while held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Hold,
    Pulse,
    PulseChange,
    Toggle,
    Repeat,
}

/// Gesture timing in milliseconds. Fields added after version 2 first shipped default when absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    pub pulse_ms: u32,
    #[serde(default = "long_ms")]
    pub long_ms: u32,
    #[serde(default = "double_ms")]
    pub double_ms: u32,
    #[serde(default = "repeat_ms")]
    pub repeat_ms: u32,
}

fn long_ms() -> u32 {
    500
}

fn double_ms() -> u32 {
    250
}

fn repeat_ms() -> u32 {
    120
}

impl Default for Timing {
    fn default() -> Self {
        Self { pulse_ms: 80, long_ms: long_ms(), double_ms: double_ms(), repeat_ms: repeat_ms() }
    }
}

/// NEO buttons and an optional POV direction: what a long press or double tap sends.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub buttons: Vec<u8>,
    #[serde(default)]
    pub pov: Option<Direction>,
}

/// Mappings used instead of the normal ones while the shift control is held.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub buttons: BTreeMap<String, Vec<u8>>,
    pub pov: BTreeMap<String, Direction>,
}

/// Camera ramp (0 is instant), full deflection once the hat is held for a long press, and the spare quadrant lever
/// (the one that is not the throttle) panning the camera.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub ramp_ms: u32,
    pub full_after_hold: bool,
    pub lever: bool,
}

/// Head tracking from OpenTrack's "UDP over network" output on 127.0.0.1: head yaw and pitch (degrees, full look at
/// the ranges) move the camera through `tuning`. The hat wins while pressed; without a sample for `timeout_ms` the
/// camera falls back to the hat. Port and enabling take effect when emulation starts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadTracking {
    pub enabled: bool,
    pub port: u16,
    pub yaw_range: f64,
    pub pitch_range: f64,
    pub invert_yaw: bool,
    pub invert_pitch: bool,
    pub timeout_ms: u32,
    pub tuning: AxisTuning,
}

impl Default for HeadTracking {
    fn default() -> Self {
        let tuning = AxisTuning { deadzone: 0.03, ..AxisTuning::default() };
        Self { enabled: false, port: 4242, yaw_range: 45.0, pitch_range: 30.0, invert_yaw: false, invert_pitch: false, timeout_ms: 500, tuning }
    }
}

/// What drives NEO yaw: the sidestick twist, nothing, or the difference between the two quadrant levers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YawSource {
    #[default]
    Twist,
    Off,
    LeverDifferential,
}

fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invert {
    pub roll: bool,
    pub pitch: bool,
    pub yaw: bool,
    pub throttle: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub axes: Axes,
    pub camera_strength: f64,
    pub throttle_lever: Lever,
    /// Absent in profiles saved before the setting existed: camera.
    #[serde(default)]
    pub hat: HatMode,
    /// Absent in profiles saved before the setting existed: enabled at 75% / 25%.
    #[serde(default)]
    pub split_high_g: SplitHighG,
    pub invert: Invert,
    #[serde(default)]
    pub throttle: Throttle,
    #[serde(default)]
    pub precision: Precision,
    /// Hold the throttle at centre until the lever reaches it, so a lever left forward cannot launch at full power.
    #[serde(default = "enabled")]
    pub safe_start: bool,
    #[serde(default)]
    pub combos: Combos,
    /// Sources not listed hold their targets while pressed.
    #[serde(default)]
    pub behaviours: BTreeMap<String, Mode>,
    #[serde(default)]
    pub timing: Timing,
    /// Sent instead of the normal mapping when the source is held for `timing.long_ms`.
    #[serde(default)]
    pub long_press: BTreeMap<String, Target>,
    /// Sent when the source is pressed twice within `timing.double_ms`.
    #[serde(default)]
    pub double_tap: BTreeMap<String, Target>,
    /// While this source is held, other sources use their `shifted` mapping where they have one. Its own mappings are
    /// kept but not sent while it is the shift control.
    #[serde(default)]
    pub shift: Option<String>,
    #[serde(default)]
    pub shifted: Layer,
    #[serde(default)]
    pub camera: Camera,
    #[serde(default)]
    pub yaw_source: YawSource,
    #[serde(default)]
    pub head_tracking: HeadTracking,
    pub buttons: BTreeMap<String, Vec<u8>>,
    pub pov: BTreeMap<String, Direction>,
}

pub const VERSION: u32 = 2;

/// Default profile: the mapping recorded on the Mapping tab on 17 October 2026. Sidestick 1–15 send the
/// same-numbered NEO buttons; quadrant grips 1–2 High-G; ENG switches 3–4 and button 6 Autopilot; 5 Radar;
/// mode selector 7–8 Flares/View. No POV source; the hat drives the camera. Throttle not inverted
/// (checked in AC8); split-lever High-G at 75% / 25% (the settings tuned on the real quadrant). The latching ENG
/// switches pulse Autopilot on every change and the mode selector pulses Flares/View once, instead of holding them.
pub fn defaults() -> Config {
    let buttons = [
        ("s1", 1), ("s2", 2), ("s3", 3), ("s4", 4), ("s5", 5), ("s6", 6), ("s7", 7), ("s8", 8),
        ("s9", 9), ("s10", 10), ("s11", 11), ("s12", 12), ("s13", 13), ("s14", 14), ("s15", 15),
        ("q1", 5), ("q2", 5), ("q3", 6), ("q4", 6), ("q5", 9), ("q6", 6), ("q7", 7), ("q8", 8),
    ];
    Config {
        version: VERSION,
        axes: Axes::default(),
        camera_strength: 1.0,
        throttle_lever: Lever::Left,
        hat: HatMode::Camera,
        split_high_g: SplitHighG::default(),
        invert: Invert::default(),
        throttle: Throttle::default(),
        precision: Precision::default(),
        safe_start: true,
        combos: Combos::default(),
        behaviours: [("q3", Mode::PulseChange), ("q4", Mode::PulseChange), ("q7", Mode::Pulse), ("q8", Mode::Pulse)].iter().map(|(k, m)| (k.to_string(), *m)).collect(),
        timing: Timing::default(),
        long_press: BTreeMap::new(),
        double_tap: BTreeMap::new(),
        shift: None,
        shifted: Layer::default(),
        camera: Camera::default(),
        yaw_source: YawSource::Twist,
        head_tracking: HeadTracking::default(),
        buttons: buttons.iter().map(|(k, v)| (k.to_string(), vec![*v])).collect(),
        pov: BTreeMap::new(),
    }
}

/// "s1".."s17" (sidestick), "q1".."q31" (quadrant) or "hat-up/right/down/left" to a zero-based index.
pub fn source(key: &str) -> Option<(Device, usize)> {
    if let Some(direction) = key.strip_prefix("hat-") {
        return HAT_DIRECTIONS.iter().position(|d| *d == direction).map(|i| (Device::Hat, i));
    }
    let (device, count) = match key.as_bytes().first() {
        Some(b's') => (Device::Stick, 17),
        Some(b'q') => (Device::Quadrant, 31),
        _ => return None,
    };
    let digits = &key[1..];
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number: usize = digits.parse().ok()?;
    (1..=count).contains(&number).then_some((device, number - 1))
}

fn within(value: f64, low: f64, high: f64) -> bool {
    (low..=high).contains(&value)
}

fn validate_axis(name: &str, tuning: &AxisTuning) -> Result<(), String> {
    if !within(tuning.deadzone, 0.0, 0.5) {
        return Err(format!("{name} deadzone must be between 0 and 50%."));
    }
    if !within(tuning.saturation, 0.5, 1.0) || tuning.saturation <= tuning.deadzone {
        return Err(format!("{name} full deflection must be between 50 and 100% and beyond the deadzone."));
    }
    if !within(tuning.anti_deadzone, 0.0, 0.5) {
        return Err(format!("{name} anti-deadzone must be between 0 and 50%."));
    }
    if !within(tuning.curve, -1.0, 1.0) {
        return Err(format!("{name} curve must be between -100 and 100%."));
    }
    if !within(tuning.smoothing, 0.0, 0.9) {
        return Err(format!("{name} smoothing must be between 0 and 90%."));
    }
    Ok(())
}

fn known(key: &str) -> Result<(), String> {
    source(key).map(|_| ()).ok_or_else(|| format!("Unknown TCA control {key}."))
}

pub fn validate(config: &Config) -> Result<(), String> {
    if config.version != VERSION {
        return Err("Unsupported profile version.".into());
    }
    validate_axis("Roll", &config.axes.roll)?;
    validate_axis("Pitch", &config.axes.pitch)?;
    validate_axis("Yaw", &config.axes.yaw)?;
    if !(0.0..=1.0).contains(&config.camera_strength) {
        return Err("Camera strength must be between 0 and 1.".into());
    }
    let split = &config.split_high_g;
    if !(0.0..=1.0).contains(&split.back) || !(0.0..=1.0).contains(&split.forward) || split.back >= split.forward {
        return Err("Split High-G thresholds need 0 ≤ other lever < forward lever ≤ 100%.".into());
    }
    let t = &config.throttle;
    if !within(t.band, 0.0, 0.2) || t.idle < 0.0 || t.idle >= t.cruise - t.band || t.cruise + t.band >= t.boost || t.boost > 1.0 {
        return Err("Throttle detents need 0 ≤ idle < cruise − band, cruise + band < boost ≤ 100%, and a band up to 20%.".into());
    }
    config.precision.source.iter().try_for_each(|key| known(key))?;
    if !within(config.precision.scale, 0.1, 1.0) {
        return Err("Precision scale must be between 10 and 100%.".into());
    }
    for combo in [&config.combos.pause, &config.combos.next_profile] {
        if combo.len() > 4 {
            return Err("A combo has at most 4 controls.".into());
        }
        combo.iter().try_for_each(|key| known(key))?;
    }
    config.behaviours.keys().try_for_each(|key| known(key))?;
    let timing = &config.timing;
    if !(20..=1000).contains(&timing.pulse_ms) {
        return Err("Pulse length must be between 20 and 1000 ms.".into());
    }
    if !(200..=2000).contains(&timing.long_ms) || !(100..=1000).contains(&timing.double_ms) {
        return Err("A long press needs 200 to 2000 ms and a double tap window 100 to 1000 ms.".into());
    }
    if !(30..=1000).contains(&timing.repeat_ms) || timing.repeat_ms <= timing.pulse_ms {
        return Err("Repeat interval must be 30 to 1000 ms and longer than the pulse.".into());
    }
    let buttons = config.buttons.iter().chain(&config.shifted.buttons);
    let gestures = config.long_press.iter().chain(&config.double_tap).map(|(key, target)| (key, &target.buttons));
    for (key, targets) in buttons.chain(gestures) {
        known(key)?;
        if targets.iter().any(|t| !(1..=15).contains(t)) {
            return Err(format!("{key}: NEO buttons are numbered 1 to 15."));
        }
    }
    config.pov.keys().chain(config.shifted.pov.keys()).try_for_each(|key| known(key))?;
    config.shift.iter().try_for_each(|key| known(key))?;
    let head = &config.head_tracking;
    if head.port < 1024 || !within(head.yaw_range, 5.0, 180.0) || !within(head.pitch_range, 5.0, 90.0) || !(100..=5000).contains(&head.timeout_ms) {
        return Err("Head tracking needs a port from 1024, look ranges of 5 to 180° (yaw) and 5 to 90° (pitch), and a 100 to 5000 ms timeout.".into());
    }
    validate_axis("Head tracking", &head.tuning)?;
    if config.camera.ramp_ms > 2000 {
        return Err("Camera ramp must be at most 2000 ms.".into());
    }
    let levers = [config.split_high_g.enabled, config.camera.lever, config.yaw_source == YawSource::LeverDifferential];
    if levers.iter().filter(|&&used| used).count() > 1 {
        return Err("Split-lever High-G, the camera lever and lever yaw each need the quadrant levers: turn on only one.".into());
    }
    Ok(())
}

/// Version 1 kept one deadzone for roll, pitch and yaw; version 2 tunes each axis. Other v1 settings carry over,
/// and v1 sources keep holding their targets (no behaviours), so nothing changes silently.
fn migrate(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut().filter(|o| o.get("version") == Some(&1.into())) {
        let deadzone = object.remove("deadzone").unwrap_or(serde_json::Value::Null);
        let axis = serde_json::json!({"deadzone": deadzone, "saturation": 1.0, "anti_deadzone": 0.0, "curve": 0.0, "smoothing": 0.0});
        object.insert("axes".into(), serde_json::json!({"roll": axis, "pitch": axis, "yaw": axis}));
        object.insert("behaviours".into(), serde_json::json!({}));
        object.insert("version".into(), VERSION.into());
    }
    value
}

pub fn parse(text: &str) -> Result<Config, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("Invalid profile: {e}."))?;
    let config: Config = serde_json::from_value(migrate(value)).map_err(|e| format!("Invalid profile: {e}."))?;
    validate(&config)?;
    Ok(config)
}

pub fn to_json(config: &Config) -> String {
    serde_json::to_string_pretty(config).expect("profile serializes")
}

/// A missing profile means research defaults; an unreadable one is an error, never silently replaced.
pub fn load(path: &Path) -> Result<Config, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(defaults()),
        Err(_) => Err("The saved profile could not be read.".into()),
    }
}

pub fn save(path: &Path, config: &Config) -> Result<(), String> {
    validate(config)?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, to_json(config))
        .and_then(|()| std::fs::rename(&temporary, path))
        .map_err(|_| "The profile could not be saved.".to_string())
}
