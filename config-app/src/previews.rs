//! The settings pages' live previews, as numbers: every one goes through the daemon's own maths (`neo_common::maths`),
//! so what the page shows is what the NEO receives.
use crate::mapping::short;
use neo_common::api::Monitor;
use neo_common::config::{Camera, Config, HatMode, HeadTracking, Lever, SplitHighG, Throttle, ThrottleMode, Timing, YawSource};
use neo_common::maths::{detent, shape};

/// "+0.40", "−0.40", " 0.00".
pub fn signed(value: f64) -> String {
    let text = format!("{:.2}", value.abs());
    let sign = match value {
        _ if text == "0.00" => ' ',
        v if v > 0.0 => '+',
        _ => '\u{2212}',
    };
    format!("{sign}{text}")
}

/// Delay that smoothing adds at the 10 ms report period, in whole milliseconds.
pub fn lag(smoothing: f64) -> u32 {
    (10.0 * smoothing / (1.0 - smoothing)).round() as u32
}

/// What the game reads for a lever travel: -1 brake … +1 boost.
pub fn throttle_out(travel: f64, throttle: &Throttle) -> f64 {
    match throttle.mode {
        ThrottleMode::Detents => detent(travel, throttle),
        ThrottleMode::Linear => 2.0 * travel - 1.0,
    }
}

/// The throttle band an output falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Brake,
    Cruise,
    Boost,
}

/// The throttle readout: its band (detents only) and its name.
pub fn zone(out: f64, throttle: &Throttle) -> (Option<Zone>, String) {
    if throttle.mode == ThrottleMode::Linear {
        return (None, format!("{}%", ((out + 1.0) * 50.0).round()));
    }
    let (zone, name) = if out <= -1.0 {
        (Zone::Brake, "Full brake")
    } else if out < 0.0 {
        (Zone::Brake, "Braking")
    } else if out == 0.0 {
        (Zone::Cruise, "Cruise")
    } else if out >= 1.0 {
        (Zone::Boost, "Full boost")
    } else {
        (Zone::Boost, "Boosting")
    };
    (Some(zone), name.into())
}

/// The response curve from full left to full right: 121 (input, output) points.
pub fn curve(tuning: &neo_common::config::AxisTuning) -> Vec<(f64, f64)> {
    (0..=120).map(|i| f64::from(i) / 60.0 - 1.0).map(|x| (x, shape(x, tuning))).collect()
}

/// The camera ramp graph spans this long a hat hold.
pub const RAMP_SPAN_MS: u32 = 1500;

/// The camera output while the hat is held, every 25 ms over the graph: (ms, output 0..1). Same rule as the daemon.
pub fn ramp(camera: &Camera, strength: f64, long_ms: u32) -> Vec<(u32, f64)> {
    (0..=RAMP_SPAN_MS)
        .step_by(25)
        .map(|t| {
            let ramp = if camera.ramp_ms == 0 { 1.0 } else { (f64::from(t) / f64::from(camera.ramp_ms)).min(1.0) };
            (t, if camera.full_after_hold && t >= long_ms { 1.0 } else { strength * ramp })
        })
        .collect()
}

/// Camera deflection (-1..1, NEO convention: up is negative Y) from a head sample (degrees), as the daemon computes it.
pub fn head_camera([yaw, pitch]: [f64; 2], tracking: &HeadTracking) -> [f64; 2] {
    let sign = |invert: bool| if invert { -1.0 } else { 1.0 };
    [shape(sign(tracking.invert_yaw) * yaw / tracking.yaw_range, &tracking.tuning), shape(-sign(tracking.invert_pitch) * pitch / tracking.pitch_range, &tracking.tuning)]
}

/// "+12°", "−3°", "0°".
pub fn degrees(value: f64) -> String {
    let rounded = value.round();
    let sign = match rounded {
        r if r > 0.0 => "+",
        r if r < 0.0 => "\u{2212}",
        _ => "",
    };
    format!("{sign}{}\u{b0}", rounded.abs())
}

/// True while the split-lever rule holds NEO 5.
pub fn split_active([left, right]: [f64; 2], rule: &SplitHighG) -> bool {
    rule.enabled && left.max(right) >= rule.forward && left.min(right) <= rule.back
}

/// The live lever readout, flagging an active split against the draft thresholds.
pub fn lever_text(levers: [f64; 2], rule: &SplitHighG) -> String {
    let active = if split_active(levers, rule) { " \u{b7} High-G active" } else { "" };
    format!("Left lever {}% \u{b7} Right lever {}%{active}", (levers[0] * 100.0).round(), (levers[1] * 100.0).round())
}

/// One press of a control, in milliseconds of the window's clock; `end` is None while it is held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Press {
    pub start: f64,
    pub end: Option<f64>,
}

/// How a press reads against the timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// A short press at release.
    Tap,
    /// Held from the long-press time.
    Long,
    /// A short press on the second tap.
    Double,
}

/// What one control's presses send: (gesture, from, to) in the presses' clock. A press still held below the
/// long-press time reads as nothing yet.
pub fn gestures(presses: &[Press], timing: &Timing, now: f64) -> Vec<(Gesture, f64, f64)> {
    let (long, double, pulse) = (f64::from(timing.long_ms), f64::from(timing.double_ms), f64::from(timing.pulse_ms));
    let mut out: Vec<(Gesture, f64, f64)> = Vec::new();
    // The last tap, if the next press could still make it a double tap: (its release, its index in `out`).
    let mut tap: Option<(f64, usize)> = None;
    for press in presses {
        let until = press.end.unwrap_or(now);
        match press.end {
            _ if until - press.start >= long => {
                out.push((Gesture::Long, press.start + long, until));
                tap = None;
            }
            Some(end) => match tap {
                Some((released, index)) if press.start - released <= double => {
                    out[index] = (Gesture::Double, press.start, press.start + pulse);
                    tap = None;
                }
                _ => {
                    tap = Some((end, out.len()));
                    out.push((Gesture::Tap, end, end + pulse));
                }
            },
            None => {}
        }
    }
    out
}

/// A state chip's colour: plain, ok (green), warn (amber) or accent (cyan).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Ok,
    Warn,
    Accent,
}

/// The throttle zone's chip colour.
pub fn zone_tone(zone: Option<Zone>) -> Tone {
    match zone {
        None => Tone::Plain,
        Some(Zone::Cruise) => Tone::Ok,
        Some(Zone::Brake) => Tone::Warn,
        Some(Zone::Boost) => Tone::Accent,
    }
}

/// A NEO axis value (0..65535, centre 32768) as -1..1.
pub fn axis_value(raw: u16) -> f64 {
    f64::from(raw) / 65535.0 * 2.0 - 1.0
}

/// Precision aiming's state chip.
pub fn precision_state(config: &Config, monitor: Option<&Monitor>) -> (String, Tone) {
    match (&config.precision.source, monitor.is_some_and(|m| m.precision)) {
        (None, _) => ("Precision off".into(), Tone::Plain),
        (Some(_), true) => (format!("Precision held \u{b7} {}%", (config.precision.scale * 100.0).round()), Tone::Accent),
        (Some(_), false) => ("Ready".into(), Tone::Plain),
    }
}

/// The shift layer's state chip.
pub fn shift_state(config: &Config, monitor: Option<&Monitor>) -> (String, Tone) {
    match (&config.shift, monitor.is_some_and(|m| m.shift)) {
        (None, _) => ("Shift off".into(), Tone::Plain),
        (Some(_), true) => ("Shift held".into(), Tone::Warn),
        (Some(_), false) => ("Ready".into(), Tone::Plain),
    }
}

/// Safe start's state chip.
pub fn safe_state(config: &Config, monitor: Option<&Monitor>) -> (String, Tone) {
    match (config.safe_start, monitor) {
        (false, _) => ("Off".into(), Tone::Plain),
        (true, None) => ("Starts with emulation".into(), Tone::Plain),
        (true, Some(m)) if m.armed => ("Throttle armed".into(), Tone::Ok),
        (true, Some(_)) => ("Waiting for centre".into(), Tone::Warn),
    }
}

/// The combos' state chip: a whole combo held, or how far each is.
pub fn combo_state(config: &Config, held: &[String]) -> (String, Tone) {
    let progress: Vec<(&str, usize, usize)> = [("pause", &config.combos.pause), ("next-profile", &config.combos.next_profile)]
        .into_iter()
        .map(|(name, keys)| (name, keys.len(), keys.iter().filter(|key| held.contains(key)).count()))
        .filter(|(_, keys, held)| *keys > 0 && *held > 0)
        .collect();
    if let Some((name, ..)) = progress.iter().find(|(_, keys, held)| held == keys) {
        return (format!("{}{} combo held", name[..1].to_uppercase(), &name[1..]), Tone::Ok);
    }
    if progress.is_empty() {
        return ("No combo held".into(), Tone::Plain);
    }
    (progress.iter().map(|(name, keys, held)| format!("{name} {held}/{keys}")).collect::<Vec<_>>().join(" \u{b7} "), Tone::Warn)
}

/// The hat's state chip on the camera page.
pub fn hat_state(saved_hat: HatMode, held_ms: Option<f64>) -> (String, Tone) {
    match (saved_hat, held_ms) {
        (HatMode::Mapped, _) => ("Hat mapped as buttons".into(), Tone::Plain),
        (HatMode::Camera, Some(ms)) => (format!("Hat held \u{b7} {} ms", ms.round()), Tone::Ok),
        (HatMode::Camera, None) => ("Hat released".into(), Tone::Plain),
    }
}

/// Head tracking's status: off, receiving, or waiting (for OpenTrack, or for emulation to start).
pub fn head_state(tracking: &HeadTracking, sample: Option<[f64; 2]>, emulating: bool) -> (String, Tone) {
    match (tracking.enabled, sample, emulating) {
        (false, ..) => ("Off".into(), Tone::Plain),
        (true, Some(_), _) => ("Receiving from OpenTrack".into(), Tone::Ok),
        (true, None, true) => (format!("Waiting for OpenTrack on UDP port {}\u{2026}", tracking.port), Tone::Warn),
        (true, None, false) => ("Starts with emulation".into(), Tone::Warn),
    }
}

/// The split-lever rule's state chip.
pub fn lever_state(rule: &SplitHighG, levers: Option<[f64; 2]>) -> (String, Tone) {
    match (rule.enabled, levers.is_some_and(|levers| split_active(levers, rule))) {
        (false, _) => ("Rule off".into(), Tone::Plain),
        (true, true) => ("High-G active \u{b7} NEO 5 held".into(), Tone::Warn),
        (true, false) => ("Not split".into(), Tone::Plain),
    }
}

/// What the latest control's presses read as, in words.
pub fn press_text(key: Option<&str>, presses: &[Press], timing: &Timing, now: f64) -> String {
    let (Some(key), Some(latest)) = (key, presses.last()) else {
        return "Press a TCA control: the last 3 seconds are drawn against the long-press and double-tap times.".into();
    };
    let held = (latest.end.unwrap_or(now) - latest.start).round();
    let what = match gestures(presses, timing, now).last().map(|read| read.0) {
        _ if latest.end.is_none() && held < f64::from(timing.long_ms) => format!("holding {held} ms\u{2026}"),
        Some(Gesture::Long) => format!("long press: held {held} ms (long press from {} ms)", timing.long_ms),
        Some(Gesture::Double) => format!("double tap: second tap within {} ms", timing.double_ms),
        _ => format!("tap: {held} ms"),
    };
    format!("{} \u{b7} {what}", short(key))
}
/// One row of the Live page: the axis, its source, the raw input and what the NEO receives (both -1..1). The
/// throttle reads as the game does (+1 boost), the camera from the hat (and the spare lever).
pub fn live_rows(monitor: &Monitor, pressed: &[String], config: &Config) -> [(&'static str, &'static str, f64, f64); 6] {
    let right = config.throttle_lever == Lever::Right;
    let differential = config.yaw_source == YawSource::LeverDifferential;
    let travel = if differential { (monitor.levers[0] + monitor.levers[1]) / 2.0 } else { monitor.levers[usize::from(right)] };
    let held = |key: &str| f64::from(u8::from(pressed.iter().any(|p| p == key)));
    let hat_x = held("hat-right") - held("hat-left");
    let spare = 2.0 * monitor.levers[usize::from(!right)] - 1.0;
    let camera_x = if config.camera.lever && hat_x == 0.0 { spare } else { hat_x };
    let (yaw_source, yaw) = match config.yaw_source {
        YawSource::Twist => ("Twist", monitor.yaw),
        YawSource::Off => ("Off", 0.0),
        YawSource::LeverDifferential => ("Lever difference", monitor.levers[0] - monitor.levers[1]),
    };
    let lever = match (differential, right) {
        (true, _) => "Both levers",
        (false, true) => "Right lever",
        (false, false) => "Left lever",
    };
    [
        ("Roll", "Stick X", monitor.roll, axis_value(monitor.axes[0])),
        ("Pitch", "Stick Y", monitor.pitch, axis_value(monitor.axes[1])),
        ("Yaw", yaw_source, yaw, axis_value(monitor.axes[5])),
        ("Throttle", lever, 2.0 * travel - 1.0, -axis_value(monitor.axes[2])),
        ("Camera X", if config.camera.lever { "Hat or spare lever" } else { "Hat" }, camera_x, axis_value(monitor.axes[3])),
        ("Camera Y", "Hat", held("hat-down") - held("hat-up"), axis_value(monitor.axes[4])),
    ]
}

/// The Live page's five state tiles: (text, tone).
pub fn live_states(monitor: Option<&Monitor>, paused: bool, previewing: bool) -> [(&'static str, Tone); 5] {
    let pick = |on: bool, yes: (&'static str, Tone), no: (&'static str, Tone)| if on { yes } else { no };
    [
        pick(monitor.is_none_or(|m| m.armed), ("Throttle armed", Tone::Ok), ("Throttle waiting for centre", Tone::Warn)),
        pick(monitor.is_some_and(|m| m.precision), ("Precision on", Tone::Warn), ("Precision off", Tone::Plain)),
        pick(monitor.is_some_and(|m| m.shift), ("Shift held", Tone::Warn), ("Shift off", Tone::Plain)),
        pick(paused, ("Input paused", Tone::Warn), ("Input live", Tone::Ok)),
        pick(previewing, ("Previewing unsaved", Tone::Warn), ("Preview off", Tone::Plain)),
    ]
}

/// The POV as sent (8 is centred): its name, and its cell in a 3 × 3 grid read left to right, top to bottom.
pub fn pov(value: u8) -> (String, Option<usize>) {
    const CELLS: [usize; 8] = [1, 2, 5, 8, 7, 6, 3, 0];
    match neo_common::maths::HAT_NAMES.get(usize::from(value)) {
        Some(name) => (format!("POV {name}"), Some(CELLS[usize::from(value)])),
        None => ("POV centred".into(), None),
    }
}