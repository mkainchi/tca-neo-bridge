//! Axis and throttle maths shared by the emulation and the configuration app's previews, so both agree exactly.
use crate::config::{AxisTuning, Throttle};

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

pub const HAT_NAMES: [&str; 8] = ["up", "up-right", "right", "down-right", "down", "down-left", "left", "up-left"];

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
