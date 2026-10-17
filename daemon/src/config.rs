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
    pub deadzone: f64,
    pub camera_strength: f64,
    pub throttle_lever: Lever,
    /// Absent in profiles saved before the setting existed: camera.
    #[serde(default)]
    pub hat: HatMode,
    /// Absent in profiles saved before the setting existed: enabled at 75% / 25%.
    #[serde(default)]
    pub split_high_g: SplitHighG,
    pub invert: Invert,
    pub buttons: BTreeMap<String, Vec<u8>>,
    pub pov: BTreeMap<String, Direction>,
}

/// Default profile: the mapping recorded on the Mapping tab on 17 October 2026. Sidestick 1–15 send the
/// same-numbered NEO buttons; quadrant grips 1–2 High-G; ENG switches 3–4 and button 6 Autopilot; 5 Radar;
/// mode selector 7–8 Flares/View. No POV source; the hat drives the camera. Throttle not inverted
/// (checked in AC8); split-lever High-G at 75% / 25% (the settings tuned on the real quadrant).
pub fn defaults() -> Config {
    let buttons = [
        ("s1", 1), ("s2", 2), ("s3", 3), ("s4", 4), ("s5", 5), ("s6", 6), ("s7", 7), ("s8", 8),
        ("s9", 9), ("s10", 10), ("s11", 11), ("s12", 12), ("s13", 13), ("s14", 14), ("s15", 15),
        ("q1", 5), ("q2", 5), ("q3", 6), ("q4", 6), ("q5", 9), ("q6", 6), ("q7", 7), ("q8", 8),
    ];
    Config {
        version: 1,
        deadzone: 0.04,
        camera_strength: 1.0,
        throttle_lever: Lever::Left,
        hat: HatMode::Camera,
        split_high_g: SplitHighG::default(),
        invert: Invert::default(),
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

pub fn validate(config: &Config) -> Result<(), String> {
    if config.version != 1 {
        return Err("Unsupported profile version.".into());
    }
    if !(0.0..1.0).contains(&config.deadzone) {
        return Err("Deadzone must be at least 0 and below 1.".into());
    }
    if !(0.0..=1.0).contains(&config.camera_strength) {
        return Err("Camera strength must be between 0 and 1.".into());
    }
    let split = &config.split_high_g;
    if !(0.0..=1.0).contains(&split.back) || !(0.0..=1.0).contains(&split.forward) || split.back >= split.forward {
        return Err("Split High-G thresholds need 0 ≤ other lever < forward lever ≤ 100%.".into());
    }
    for (key, targets) in &config.buttons {
        if source(key).is_none() {
            return Err(format!("Unknown TCA control {key}."));
        }
        if targets.iter().any(|t| !(1..=15).contains(t)) {
            return Err(format!("{key}: NEO buttons are numbered 1 to 15."));
        }
    }
    if let Some(key) = config.pov.keys().find(|key| source(key).is_none()) {
        return Err(format!("Unknown TCA control {key}."));
    }
    Ok(())
}

pub fn parse(text: &str) -> Result<Config, String> {
    let config: Config = serde_json::from_str(text).map_err(|e| format!("Invalid profile: {e}."))?;
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
