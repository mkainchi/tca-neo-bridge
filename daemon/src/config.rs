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
}

/// How a source drives its NEO targets: held while pressed, a short press on switch-on, a short press on every
/// change, or flipped on each press.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Hold,
    Pulse,
    PulseChange,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    pub pulse_ms: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Self { pulse_ms: 80 }
    }
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
    if config.combos.pause.len() > 4 {
        return Err("A pause combo has at most 4 controls.".into());
    }
    config.combos.pause.iter().try_for_each(|key| known(key))?;
    config.behaviours.keys().try_for_each(|key| known(key))?;
    if !(20..=1000).contains(&config.timing.pulse_ms) {
        return Err("Pulse length must be between 20 and 1000 ms.".into());
    }
    for (key, targets) in &config.buttons {
        known(key)?;
        if targets.iter().any(|t| !(1..=15).contains(t)) {
            return Err(format!("{key}: NEO buttons are numbered 1 to 15."));
        }
    }
    config.pov.keys().try_for_each(|key| known(key))
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
