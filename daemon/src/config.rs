//! Private NEO mapping profile: validation, defaults, v1 (TCA-only) migration and atomic persistence.
//!
//! Version 2 keeps one mapping per controller set ("setup", the sorted device ids joined by "+"), so each HOTAS
//! keeps its own mapping. Sources are "<device>/b<n>" (button n) and "<device>/hat-<up|right|down|left>".
use crate::models::{self, DeviceInfo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const VERSION: u32 = 2;
/// Setup key of the TCA Airbus sidestick + quadrant pair.
pub const TCA_SETUP: &str = "tca-quadrant+tca-stick";

/// Mappable hat directions, in "hat-<direction>" source keys; diagonals press two of them.
pub const HAT_DIRECTIONS: [&str; 4] = ["up", "right", "down", "left"];

/// What the camera hat drives: the NEO camera mini-stick (research default) or its own mappings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HatMode {
    #[default]
    Camera,
    Mapped,
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

/// One device axis feeding a NEO axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub device: String,
    pub axis: String,
    #[serde(default)]
    pub invert: bool,
}

impl Binding {
    pub fn new(device: &str, axis: &str, invert: bool) -> Self {
        Self { device: device.into(), axis: axis.into(), invert }
    }
}

/// Roll, pitch and yaw take one axis each; every throttle axis shares the throttle (the last one moved wins).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axes {
    #[serde(default)]
    pub roll: Option<Binding>,
    #[serde(default)]
    pub pitch: Option<Binding>,
    #[serde(default)]
    pub yaw: Option<Binding>,
    #[serde(default)]
    pub throttle: Vec<Binding>,
}

/// Hold NEO 5 (High-G) while one lever is at or above `forward` and the other at or below `back` (fractions of
/// travel, either order); the throttle follows the forward lever meanwhile, with that lever's inversion. Travel is
/// 0 at the low raw end and 1 at the high raw end when `reverse`, the other way round otherwise (the TCA convention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitHighG {
    pub enabled: bool,
    pub forward: f64,
    pub back: f64,
    #[serde(default)]
    pub levers: Vec<Binding>,
    #[serde(default)]
    pub reverse: bool,
}

impl Default for SplitHighG {
    fn default() -> Self {
        Self { enabled: false, forward: 0.75, back: 0.25, levers: Vec::new(), reverse: false }
    }
}

/// The mapping of one controller set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    #[serde(default)]
    pub hat: HatMode,
    /// Device whose hat drives the camera; None: the first device with a hat.
    #[serde(default)]
    pub camera: Option<String>,
    #[serde(default)]
    pub axes: Axes,
    #[serde(default)]
    pub split_high_g: SplitHighG,
    pub buttons: BTreeMap<String, Vec<u8>>,
    pub pov: BTreeMap<String, Direction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub deadzone: f64,
    pub camera_strength: f64,
    /// Device ids taking part; empty: every Thrustmaster joystick present.
    #[serde(default)]
    pub devices: Vec<String>,
    #[serde(default)]
    pub setups: BTreeMap<String, Profile>,
}

impl Config {
    /// The saved mapping of this controller set, or its default.
    pub fn profile(&self, devices: &[DeviceInfo]) -> Profile {
        self.setups.get(&models::setup_key(devices)).cloned().unwrap_or_else(|| default_profile(devices))
    }
}

pub fn defaults() -> Config {
    Config { version: VERSION, deadzone: 0.04, camera_strength: 1.0, devices: Vec::new(), setups: BTreeMap::new() }
}

/// TCA Airbus default: the mapping recorded on the Mapping tab on 17 October 2026. Sidestick 1–15 send the
/// same-numbered NEO buttons; quadrant grips 1–2 High-G; ENG switches 3–4 and button 6 Autopilot; 5 Radar;
/// mode selector 7–8 Flares/View. No POV source; the hat drives the camera. Throttle not inverted (checked in
/// AC8); left lever and sidestick slider share the throttle; split-lever High-G at 75% / 25%.
pub fn tca_profile() -> Profile {
    let (s, q) = (models::TCA_STICK, models::TCA_QUADRANT);
    let mut pairs: Vec<(String, u8)> = (1..=15).map(|n| (format!("{s}/b{n}"), n)).collect();
    pairs.extend([(1, 5), (2, 5), (3, 6), (4, 6), (5, 9), (6, 6), (7, 7), (8, 8)].map(|(b, n)| (format!("{q}/b{b}"), n)));
    Profile {
        hat: HatMode::Camera,
        camera: Some(s.into()),
        axes: Axes {
            roll: Some(Binding::new(s, "x", false)),
            pitch: Some(Binding::new(s, "y", false)),
            yaw: Some(Binding::new(s, "rz", false)),
            throttle: vec![Binding::new(s, "slider", false), Binding::new(q, "left", false)],
        },
        split_high_g: SplitHighG { enabled: true, levers: vec![Binding::new(q, "left", false), Binding::new(q, "right", false)], ..SplitHighG::default() },
        buttons: pairs.into_iter().map(|(k, v)| (k, vec![v])).collect(),
        pov: BTreeMap::new(),
    }
}

/// Convention for an untested controller set: the first device with X and Y is the stick (roll, pitch, yaw from
/// its Rz or twist-like axis, buttons 1–15 → NEO 1–15); every device's first throttle-like axis feeds the throttle;
/// the stick's hat (or the first hat) drives the camera. Directions are left to the player to check.
pub fn default_profile(devices: &[DeviceInfo]) -> Profile {
    if models::setup_key(devices) == TCA_SETUP {
        return tca_profile();
    }
    let has = |d: &DeviceInfo, axis: &str| d.layout.axis(axis).is_some();
    let stick = devices.iter().find(|d| has(d, "x") && has(d, "y"));
    let mut axes = Axes::default();
    let mut buttons = BTreeMap::new();
    if let Some(stick) = stick {
        axes.roll = Some(Binding::new(&stick.id, "x", false));
        axes.pitch = Some(Binding::new(&stick.id, "y", false));
        for n in 1..=stick.layout.buttons.min(15) {
            buttons.insert(format!("{}/b{n}", stick.id), vec![n as u8]);
        }
    }
    let yaw = stick.filter(|s| has(s, "rz")).or_else(|| devices.iter().find(|d| has(d, "rz") || has(d, "rudder")));
    axes.yaw = yaw.map(|d| Binding::new(&d.id, if has(d, "rz") { "rz" } else { "rudder" }, false));
    // Throttle-like names never collide with the roll, pitch and yaw axes above.
    for device in devices {
        if let Some(axis) = ["throttle", "z", "slider"].iter().find(|a| has(device, a)) {
            axes.throttle.push(Binding::new(&device.id, axis, false));
        }
    }
    let camera = stick.filter(|s| s.layout.hat).or_else(|| devices.iter().find(|d| d.layout.hat)).map(|d| d.id.clone());
    Profile { hat: HatMode::Camera, camera, axes, split_high_g: SplitHighG::default(), buttons, pov: BTreeMap::new() }
}

/// A mapping source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// Zero-based button index.
    Button(usize),
    /// Index into HAT_DIRECTIONS.
    Hat(usize),
}

/// "<device>/b1".."<device>/b128" or "<device>/hat-up|right|down|left" → (device id, control).
pub fn source(key: &str) -> Option<(&str, Control)> {
    let (device, control) = key.rsplit_once('/')?;
    if !models::valid_id(device) {
        return None;
    }
    if let Some(direction) = control.strip_prefix("hat-") {
        return HAT_DIRECTIONS.iter().position(|d| *d == direction).map(|i| (device, Control::Hat(i)));
    }
    let digits = control.strip_prefix('b')?;
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number: usize = digits.parse().ok()?;
    (1..=128).contains(&number).then_some((device, Control::Button(number - 1)))
}

fn valid_axis(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn check_binding(binding: &Binding) -> Result<(), String> {
    if models::valid_id(&binding.device) && valid_axis(&binding.axis) {
        Ok(())
    } else {
        Err(format!("Unknown axis {}/{}.", binding.device, binding.axis))
    }
}

pub fn validate_profile(key: &str, profile: &Profile) -> Result<(), String> {
    if key.is_empty() || !key.split('+').all(models::valid_id) {
        return Err("Unknown controller set.".into());
    }
    let split = &profile.split_high_g;
    if !(0.0..=1.0).contains(&split.back) || !(0.0..=1.0).contains(&split.forward) || split.back >= split.forward {
        return Err("Split High-G thresholds need 0 ≤ other lever < forward lever ≤ 100%.".into());
    }
    if !(split.levers.is_empty() || split.levers.len() == 2) || (split.enabled && split.levers.is_empty()) {
        return Err("Split High-G needs exactly two levers.".into());
    }
    let axes = &profile.axes;
    if axes.throttle.len() > 8 {
        return Err("At most eight throttle axes.".into());
    }
    for binding in [&axes.roll, &axes.pitch, &axes.yaw].into_iter().flatten().chain(&axes.throttle).chain(&split.levers) {
        check_binding(binding)?;
    }
    if profile.camera.as_deref().is_some_and(|c| !models::valid_id(c)) {
        return Err("Unknown camera hat.".into());
    }
    for (key, targets) in &profile.buttons {
        if source(key).is_none() {
            return Err(format!("Unknown control {key}."));
        }
        if targets.iter().any(|t| !(1..=15).contains(t)) {
            return Err(format!("{key}: NEO buttons are numbered 1 to 15."));
        }
    }
    if let Some(key) = profile.pov.keys().find(|key| source(key).is_none()) {
        return Err(format!("Unknown control {key}."));
    }
    Ok(())
}

pub fn validate(config: &Config) -> Result<(), String> {
    if config.version != VERSION {
        return Err("Unsupported profile version.".into());
    }
    if !(0.0..1.0).contains(&config.deadzone) {
        return Err("Deadzone must be at least 0 and below 1.".into());
    }
    if !(0.0..=1.0).contains(&config.camera_strength) {
        return Err("Camera strength must be between 0 and 1.".into());
    }
    if config.devices.len() > 16 || !config.devices.iter().all(|d| models::valid_id(d)) {
        return Err("Unknown controller in the device list.".into());
    }
    if config.setups.len() > 32 {
        return Err("At most 32 controller sets.".into());
    }
    config.setups.iter().try_for_each(|(key, profile)| validate_profile(key, profile))
}

/// Version 1 profile of the TCA-only bridge (0.2.x), read only to migrate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigV1 {
    pub version: u32,
    pub deadzone: f64,
    pub camera_strength: f64,
    pub throttle_lever: LeverV1,
    #[serde(default)]
    pub hat: HatMode,
    #[serde(default = "split_v1")]
    pub split_high_g: SplitV1,
    pub invert: InvertV1,
    pub buttons: BTreeMap<String, Vec<u8>>,
    pub pov: BTreeMap<String, Direction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeverV1 {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitV1 {
    pub enabled: bool,
    pub forward: f64,
    pub back: f64,
}

/// Absent in v1 profiles saved before the setting existed: enabled at 75% / 25%.
fn split_v1() -> SplitV1 {
    SplitV1 { enabled: true, forward: 0.75, back: 0.25 }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvertV1 {
    pub roll: bool,
    pub pitch: bool,
    pub yaw: bool,
    pub throttle: bool,
}

/// v1 source key ("s1".."s17", "q1".."q31", "hat-up") → v2 key on the TCA devices.
pub fn migrate_key(key: &str) -> Option<String> {
    if let Some(direction) = key.strip_prefix("hat-") {
        return HAT_DIRECTIONS.contains(&direction).then(|| format!("{}/{key}", models::TCA_STICK));
    }
    let (device, count) = match key.as_bytes().first() {
        Some(b's') => (models::TCA_STICK, 17),
        Some(b'q') => (models::TCA_QUADRANT, 31),
        _ => return None,
    };
    let digits = &key[1..];
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number: usize = digits.parse().ok()?;
    (1..=count).contains(&number).then(|| format!("{device}/b{number}"))
}

/// A v1 profile becomes the TCA setup of a v2 profile with identical NEO output.
pub fn migrate(v1: ConfigV1) -> Result<Config, String> {
    if v1.version != 1 {
        return Err("Unsupported profile version.".into());
    }
    let rename = |key: &String| migrate_key(key).ok_or_else(|| format!("Unknown TCA control {key}."));
    let buttons = v1.buttons.iter().map(|(k, v)| Ok((rename(k)?, v.clone()))).collect::<Result<_, String>>()?;
    let pov = v1.pov.iter().map(|(k, v)| Ok((rename(k)?, *v))).collect::<Result<_, String>>()?;
    let (s, q, invert) = (models::TCA_STICK, models::TCA_QUADRANT, &v1.invert);
    let lever = match v1.throttle_lever {
        LeverV1::Left => "left",
        LeverV1::Right => "right",
    };
    let profile = Profile {
        hat: v1.hat,
        camera: Some(s.into()),
        axes: Axes {
            roll: Some(Binding::new(s, "x", invert.roll)),
            pitch: Some(Binding::new(s, "y", invert.pitch)),
            yaw: Some(Binding::new(s, "rz", invert.yaw)),
            throttle: vec![Binding::new(s, "slider", invert.throttle), Binding::new(q, lever, invert.throttle)],
        },
        split_high_g: SplitHighG {
            enabled: v1.split_high_g.enabled,
            forward: v1.split_high_g.forward,
            back: v1.split_high_g.back,
            levers: vec![Binding::new(q, "left", invert.throttle), Binding::new(q, "right", invert.throttle)],
            reverse: false,
        },
        buttons,
        pov,
    };
    let setups = BTreeMap::from([(TCA_SETUP.to_string(), profile)]);
    Ok(Config { version: VERSION, deadzone: v1.deadzone, camera_strength: v1.camera_strength, devices: Vec::new(), setups })
}

/// Parse a v2 profile, or migrate a v1 one; either way the result is validated.
pub fn parse(text: &str) -> Result<Config, String> {
    let invalid = |e: serde_json::Error| format!("Invalid profile: {e}.");
    let value: serde_json::Value = serde_json::from_str(text).map_err(invalid)?;
    let config = if value.get("version").and_then(serde_json::Value::as_u64) == Some(1) {
        migrate(serde_json::from_value(value).map_err(invalid)?)?
    } else {
        serde_json::from_value(value).map_err(invalid)?
    };
    validate(&config)?;
    Ok(config)
}

pub fn to_json(config: &Config) -> String {
    serde_json::to_string_pretty(config).expect("profile serializes")
}

/// A missing profile means defaults; an unreadable one is an error, never silently replaced.
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
