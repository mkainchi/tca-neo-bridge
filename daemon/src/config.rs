//! Private NEO mapping profile: validation, defaults, migration of every earlier format and atomic persistence.
//!
//! Version 3 keeps the settings shared by every controller (axis tuning, throttle detents, timing, safe start,
//! camera ramp, head tracking) at the top level, and one mapping per controller set ("setup", the sorted device ids
//! joined by "+"), so each HOTAS keeps its own mapping. Sources are "<device>/b<n>" (button n) and
//! "<device>/hat-<up|right|down|left>".
use crate::models::{self, DeviceInfo};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub const VERSION: u32 = 3;
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

    /// The same physical axis, whatever the inversion.
    pub fn same_axis(&self, other: &Binding) -> bool {
        self.device == other.device && self.axis == other.axis
    }
}

/// Roll, pitch and yaw take one axis each; every throttle axis shares the throttle (the last one moved wins).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bindings {
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
pub struct Tuning {
    pub roll: AxisTuning,
    pub pitch: AxisTuning,
    pub yaw: AxisTuning,
}

/// Linear: throttle travel maps straight to the NEO throttle. Detents: ACE COMBAT's centre-cruise model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThrottleMode {
    #[default]
    Linear,
    Detents,
}

/// Detent bands in throttle travel (0 idle, 1 fully forward): full brake up to `idle`, cruise within `band` of
/// `cruise`, full boost from `boost`, linear in between.
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

/// Gesture timing in milliseconds.
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
    #[serde(default)]
    pub buttons: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub pov: BTreeMap<String, Direction>,
}

/// Camera ramp (0 is instant) and full deflection once the hat is held for a long press.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub ramp_ms: u32,
    pub full_after_hold: bool,
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

/// What drives NEO yaw: its bound axis (the twist on most sticks), nothing, or the difference between the two split
/// levers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YawSource {
    #[default]
    Twist,
    Off,
    LeverDifferential,
}

/// The mapping of one controller set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    #[serde(default)]
    pub hat: HatMode,
    /// Device whose hat drives the camera; None: the first device with a hat.
    #[serde(default)]
    pub camera_hat: Option<String>,
    #[serde(default)]
    pub bindings: Bindings,
    #[serde(default)]
    pub split_high_g: SplitHighG,
    /// An axis panning the camera left and right around its centre; the hat wins while pressed.
    #[serde(default)]
    pub camera_lever: Option<Binding>,
    #[serde(default)]
    pub yaw_source: YawSource,
    #[serde(default)]
    pub precision: Precision,
    #[serde(default)]
    pub combos: Combos,
    /// Sources not listed hold their targets while pressed.
    #[serde(default)]
    pub behaviours: BTreeMap<String, Mode>,
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
    pub buttons: BTreeMap<String, Vec<u8>>,
    pub pov: BTreeMap<String, Direction>,
}

fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    /// Response tuning of the roll, pitch and yaw outputs, whatever axes feed them.
    pub axes: Tuning,
    pub camera_strength: f64,
    #[serde(default)]
    pub throttle: Throttle,
    /// Hold the throttle at centre until the throttle reaches it, so a lever left forward cannot launch at full power.
    #[serde(default = "enabled")]
    pub safe_start: bool,
    #[serde(default)]
    pub timing: Timing,
    #[serde(default)]
    pub camera: Camera,
    #[serde(default)]
    pub head_tracking: HeadTracking,
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
    Config {
        version: VERSION,
        axes: Tuning::default(),
        camera_strength: 1.0,
        throttle: Throttle::default(),
        safe_start: true,
        timing: Timing::default(),
        camera: Camera::default(),
        head_tracking: HeadTracking::default(),
        devices: Vec::new(),
        setups: BTreeMap::new(),
    }
}

fn empty_profile() -> Profile {
    Profile {
        hat: HatMode::Camera,
        camera_hat: None,
        bindings: Bindings::default(),
        split_high_g: SplitHighG::default(),
        camera_lever: None,
        yaw_source: YawSource::Twist,
        precision: Precision::default(),
        combos: Combos::default(),
        behaviours: BTreeMap::new(),
        long_press: BTreeMap::new(),
        double_tap: BTreeMap::new(),
        shift: None,
        shifted: Layer::default(),
        buttons: BTreeMap::new(),
        pov: BTreeMap::new(),
    }
}

/// TCA Airbus default: the mapping recorded on the Mapping page on 17 October 2026. Sidestick 1–15 send the
/// same-numbered NEO buttons; quadrant grips 1–2 High-G; ENG switches 3–4 and button 6 Autopilot; 5 Radar;
/// mode selector 7–8 Flares/View. No POV source; the hat drives the camera. Throttle not inverted (checked in
/// AC8); left lever and sidestick slider share the throttle; split-lever High-G at 75% / 25%. The latching ENG
/// switches pulse Autopilot on every change and the mode selector pulses Flares/View once, instead of holding them.
pub fn tca_profile() -> Profile {
    let (s, q) = (models::TCA_STICK, models::TCA_QUADRANT);
    let mut pairs: Vec<(String, u8)> = (1..=15).map(|n| (format!("{s}/b{n}"), n)).collect();
    pairs.extend([(1, 5), (2, 5), (3, 6), (4, 6), (5, 9), (6, 6), (7, 7), (8, 8)].map(|(b, n)| (format!("{q}/b{b}"), n)));
    let behaviours = [(3, Mode::PulseChange), (4, Mode::PulseChange), (7, Mode::Pulse), (8, Mode::Pulse)];
    Profile {
        camera_hat: Some(s.into()),
        bindings: Bindings {
            roll: Some(Binding::new(s, "x", false)),
            pitch: Some(Binding::new(s, "y", false)),
            yaw: Some(Binding::new(s, "rz", false)),
            throttle: vec![Binding::new(s, "slider", false), Binding::new(q, "left", false)],
        },
        split_high_g: SplitHighG { enabled: true, levers: vec![Binding::new(q, "left", false), Binding::new(q, "right", false)], ..SplitHighG::default() },
        behaviours: behaviours.iter().map(|(b, mode)| (format!("{q}/b{b}"), *mode)).collect(),
        buttons: pairs.into_iter().map(|(k, v)| (k, vec![v])).collect(),
        ..empty_profile()
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
    let mut bindings = Bindings::default();
    let mut buttons = BTreeMap::new();
    if let Some(stick) = stick {
        bindings.roll = Some(Binding::new(&stick.id, "x", false));
        bindings.pitch = Some(Binding::new(&stick.id, "y", false));
        for n in 1..=stick.layout.buttons.min(15) {
            buttons.insert(format!("{}/b{n}", stick.id), vec![n as u8]);
        }
    }
    let yaw = stick.filter(|s| has(s, "rz")).or_else(|| devices.iter().find(|d| has(d, "rz") || has(d, "rudder")));
    bindings.yaw = yaw.map(|d| Binding::new(&d.id, if has(d, "rz") { "rz" } else { "rudder" }, false));
    // Throttle-like names never collide with the roll, pitch and yaw axes above.
    for device in devices {
        if let Some(axis) = ["throttle", "z", "slider"].iter().find(|a| has(device, a)) {
            bindings.throttle.push(Binding::new(&device.id, axis, false));
        }
    }
    let camera_hat = stick.filter(|s| s.layout.hat).or_else(|| devices.iter().find(|d| d.layout.hat)).map(|d| d.id.clone());
    Profile { camera_hat, bindings, buttons, ..empty_profile() }
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
    source(key).map(|_| ()).ok_or_else(|| format!("Unknown control {key}."))
}

fn targets(key: &str, targets: &[u8]) -> Result<(), String> {
    known(key)?;
    if targets.iter().any(|t| !(1..=15).contains(t)) {
        return Err(format!("{key}: NEO buttons are numbered 1 to 15."));
    }
    Ok(())
}

pub fn validate_profile(key: &str, profile: &Profile) -> Result<(), String> {
    if key.is_empty() || !key.split('+').all(models::valid_id) {
        return Err("Unknown controller set.".into());
    }
    let split = &profile.split_high_g;
    if !(0.0..=1.0).contains(&split.back) || !(0.0..=1.0).contains(&split.forward) || split.back >= split.forward {
        return Err("Split High-G thresholds need 0 ≤ other lever < forward lever ≤ 100%.".into());
    }
    let differential = profile.yaw_source == YawSource::LeverDifferential;
    if !(split.levers.is_empty() || split.levers.len() == 2) || ((split.enabled || differential) && split.levers.is_empty()) {
        return Err("Split High-G and lever yaw need exactly two levers.".into());
    }
    let bindings = &profile.bindings;
    if bindings.throttle.len() > 8 {
        return Err("At most eight throttle axes.".into());
    }
    let single = [&bindings.roll, &bindings.pitch, &bindings.yaw, &profile.camera_lever].into_iter().flatten();
    for binding in single.chain(&bindings.throttle).chain(&split.levers) {
        check_binding(binding)?;
    }
    if profile.camera_hat.as_deref().is_some_and(|c| !models::valid_id(c)) {
        return Err("Unknown camera hat.".into());
    }
    // Split High-G and lever yaw read the same two levers, and a camera lever on one of them pans with it.
    let lever_camera = profile.camera_lever.as_ref().is_some_and(|c| split.levers.iter().any(|l| l.same_axis(c)));
    if [split.enabled, differential, lever_camera].iter().filter(|&&used| used).count() > 1 {
        return Err("Split-lever High-G, lever yaw and a camera lever on a split lever each need those levers: turn on only one.".into());
    }
    let buttons = profile.buttons.iter().chain(&profile.shifted.buttons);
    let gestures = profile.long_press.iter().chain(&profile.double_tap).map(|(key, target)| (key, &target.buttons));
    for (key, list) in buttons.chain(gestures) {
        targets(key, list)?;
    }
    profile.pov.keys().chain(profile.shifted.pov.keys()).chain(profile.behaviours.keys()).try_for_each(|key| known(key))?;
    profile.shift.iter().chain(&profile.precision.source).try_for_each(|key| known(key))?;
    if !within(profile.precision.scale, 0.1, 1.0) {
        return Err("Precision scale must be between 10 and 100%.".into());
    }
    for combo in [&profile.combos.pause, &profile.combos.next_profile] {
        if combo.len() > 4 {
            return Err("A combo has at most 4 controls.".into());
        }
        combo.iter().try_for_each(|key| known(key))?;
    }
    Ok(())
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
    let t = &config.throttle;
    if !within(t.band, 0.0, 0.2) || t.idle < 0.0 || t.idle >= t.cruise - t.band || t.cruise + t.band >= t.boost || t.boost > 1.0 {
        return Err("Throttle detents need 0 ≤ idle < cruise − band, cruise + band < boost ≤ 100%, and a band up to 20%.".into());
    }
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
    let head = &config.head_tracking;
    if head.port < 1024 || !within(head.yaw_range, 5.0, 180.0) || !within(head.pitch_range, 5.0, 90.0) || !(100..=5000).contains(&head.timeout_ms) {
        return Err("Head tracking needs a port from 1024, look ranges of 5 to 180° (yaw) and 5 to 90° (pitch), and a 100 to 5000 ms timeout.".into());
    }
    validate_axis("Head tracking", &head.tuning)?;
    if config.camera.ramp_ms > 2000 {
        return Err("Camera ramp must be at most 2000 ms.".into());
    }
    if config.devices.len() > 16 || !config.devices.iter().all(|d| models::valid_id(d)) {
        return Err("Unknown controller in the device list.".into());
    }
    if config.setups.len() > 32 {
        return Err("At most 32 controller sets.".into());
    }
    config.setups.iter().try_for_each(|(key, profile)| validate_profile(key, profile))
}

/// A TCA-only source key ("s1".."s17", "q1".."q31", "hat-up") → its key on the TCA devices.
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

fn take(object: &mut Map<String, Value>, key: &str) -> Value {
    object.remove(key).unwrap_or(Value::Null)
}

fn rename(key: &str) -> Result<String, String> {
    migrate_key(key).ok_or_else(|| format!("Unknown TCA control {key}."))
}

/// Rename TCA-only source keys: the keys of a map, a key, or a list of keys. Anything else is kept for serde to judge.
fn renamed(value: Value) -> Result<Value, String> {
    match value {
        Value::Object(map) => map.into_iter().map(|(k, v)| Ok((rename(&k)?, v))).collect::<Result<Map<_, _>, String>>().map(Value::Object),
        Value::String(key) => rename(&key).map(Value::String),
        Value::Array(keys) => keys.into_iter().map(renamed).collect::<Result<Vec<_>, _>>().map(Value::Array),
        other => Ok(other),
    }
}

/// Drop null members so serde defaults apply.
fn strip_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, strip_nulls(v))).collect()),
        other => other,
    }
}

/// Rename the source keys found in a member of an object (precision.source, combos.pause, …).
fn rename_member(object: &mut Value, member: &str) -> Result<(), String> {
    if let Some(value) = object.get_mut(member) {
        *value = renamed(value.take())?;
    }
    Ok(())
}

/// The TCA-only profile of the stable bridge (0.2.x version 1, or 0.3.x version 2) → version 3 with one TCA setup and
/// identical NEO output. Version 1 kept one deadzone for every axis and no behaviours.
fn migrate_tca(mut object: Map<String, Value>) -> Result<Value, String> {
    if object.get("version") == Some(&1.into()) {
        let axis = json!({"deadzone": take(&mut object, "deadzone"), "saturation": 1.0, "anti_deadzone": 0.0, "curve": 0.0, "smoothing": 0.0});
        object.insert("axes".into(), json!({"roll": axis, "pitch": axis, "yaw": axis}));
    }
    let (s, q) = (models::TCA_STICK, models::TCA_QUADRANT);
    let invert = take(&mut object, "invert");
    let flag = |name: &str| invert.get(name).cloned().unwrap_or(Value::Bool(false));
    let lever = take(&mut object, "throttle_lever");
    if lever != json!("left") && lever != json!("right") {
        return Err("Invalid profile: the throttle lever is left or right.".into());
    }
    // The stable camera lever was the quadrant lever that is not the throttle; its travel reads the other way round.
    let spare = if lever == json!("right") { "left" } else { "right" };
    let mut camera = take(&mut object, "camera");
    let camera_lever = match camera.as_object_mut().and_then(|c| c.remove("lever")) {
        Some(Value::Bool(true)) => json!({"device": q, "axis": spare, "invert": true}),
        _ => Value::Null,
    };
    if camera.is_object() {
        object.insert("camera".into(), camera);
    }
    let mut split = take(&mut object, "split_high_g");
    if split.is_null() {
        split = json!({"enabled": true, "forward": 0.75, "back": 0.25});
    }
    if let Some(rule) = split.as_object_mut() {
        rule.insert("levers".into(), json!([{"device": q, "axis": "left", "invert": flag("throttle")}, {"device": q, "axis": "right", "invert": flag("throttle")}]));
    }
    let shifted = take(&mut object, "shifted");
    let layer = |name: &str| renamed(shifted.get(name).cloned().unwrap_or(Value::Null));
    let mut profile = strip_nulls(json!({
        "hat": take(&mut object, "hat"),
        "camera_hat": s,
        "bindings": {
            "roll": {"device": s, "axis": "x", "invert": flag("roll")},
            "pitch": {"device": s, "axis": "y", "invert": flag("pitch")},
            "yaw": {"device": s, "axis": "rz", "invert": flag("yaw")},
            "throttle": [{"device": s, "axis": "slider", "invert": flag("throttle")}, {"device": q, "axis": lever, "invert": flag("throttle")}],
        },
        "split_high_g": split,
        "camera_lever": camera_lever,
        "yaw_source": take(&mut object, "yaw_source"),
        "precision": take(&mut object, "precision"),
        "combos": take(&mut object, "combos"),
        "behaviours": renamed(take(&mut object, "behaviours"))?,
        "long_press": renamed(take(&mut object, "long_press"))?,
        "double_tap": renamed(take(&mut object, "double_tap"))?,
        "shift": renamed(take(&mut object, "shift"))?,
        "shifted": {"buttons": layer("buttons")?, "pov": layer("pov")?},
        "buttons": renamed(take(&mut object, "buttons"))?,
        "pov": renamed(take(&mut object, "pov"))?,
    }));
    if let Some(precision) = profile.get_mut("precision") {
        rename_member(precision, "source")?;
    }
    if let Some(combos) = profile.get_mut("combos") {
        rename_member(combos, "pause")?;
        rename_member(combos, "next_profile")?;
    }
    object.insert("version".into(), VERSION.into());
    object.insert("setups".into(), json!({ TCA_SETUP: profile }));
    Ok(Value::Object(object))
}

/// The experimental bridge's version 2: one deadzone for every axis, `axes` and `camera` inside each setup.
fn migrate_any(mut object: Map<String, Value>) -> Value {
    let axis = json!({"deadzone": take(&mut object, "deadzone"), "saturation": 1.0, "anti_deadzone": 0.0, "curve": 0.0, "smoothing": 0.0});
    object.insert("axes".into(), json!({"roll": axis, "pitch": axis, "yaw": axis}));
    if let Some(setups) = object.get_mut("setups").and_then(Value::as_object_mut) {
        for profile in setups.values_mut().filter_map(Value::as_object_mut) {
            for (from, to) in [("axes", "bindings"), ("camera", "camera_hat")] {
                if let Some(value) = profile.remove(from) {
                    profile.insert(to.into(), value);
                }
            }
        }
    }
    object.insert("version".into(), VERSION.into());
    Value::Object(object)
}

/// Bring any earlier profile to version 3: the stable bridge's TCA-only versions 1 and 2 (recognised by their
/// `throttle_lever`), or the experimental bridge's version 2.
pub fn migrate(value: Value) -> Result<Value, String> {
    let Value::Object(object) = value else { return Ok(value) };
    match object.get("version").and_then(Value::as_u64) {
        Some(1) => migrate_tca(object),
        Some(2) if object.contains_key("throttle_lever") => migrate_tca(object),
        Some(2) => Ok(migrate_any(object)),
        _ => Ok(Value::Object(object)),
    }
}

/// Parse a profile of any version; the result is validated.
pub fn parse(text: &str) -> Result<Config, String> {
    let invalid = |e: serde_json::Error| format!("Invalid profile: {e}.");
    let value: Value = serde_json::from_str(text).map_err(invalid)?;
    let config: Config = serde_json::from_value(migrate(value)?).map_err(invalid)?;
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
