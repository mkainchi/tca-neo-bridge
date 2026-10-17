//! The Pi trial's validation stream (neo_responder.py synthetic_report, pattern=validation).
use crate::neo::{input_report, Report};

pub fn report(elapsed: f64) -> Report {
    let phase = elapsed % 4.0;
    let fraction = if phase < 2.0 { phase / 2.0 } else { (4.0 - phase) / 2.0 };
    let mut axes = [32768u16; 7];
    axes[(elapsed / 4.0) as usize % 7] = (65535.0 * fraction) as u16;
    let second = elapsed as u64;
    let buttons = if second % 16 == 15 { 0x13 } else { 1 << (second % 15) };
    input_report(axes, buttons, (second % 9) as u8)
}
