//! Daemon-to-driver pipe framing. The C driver (driver/neohid.c) implements the reader side.
use crate::neo::{Report, REPORT_LEN};

pub const PIPE_NAME: &str = r"\\.\pipe\neo-hid-v1";
pub const MAGIC: [u8; 2] = *b"NE";
pub const VERSION: u8 = 1;
pub const KIND_REPORT: u8 = 1;
pub const FRAME_LEN: usize = 4 + REPORT_LEN;

pub fn frame(report: &Report) -> [u8; FRAME_LEN] {
    let mut frame = [0u8; FRAME_LEN];
    frame[..4].copy_from_slice(&[MAGIC[0], MAGIC[1], VERSION, KIND_REPORT]);
    frame[4..].copy_from_slice(report);
    frame
}

pub fn parse(frame: &[u8]) -> Option<Report> {
    if frame.len() != FRAME_LEN || frame[..4] != [MAGIC[0], MAGIC[1], VERSION, KIND_REPORT] || frame[4] != 1 {
        return None;
    }
    frame[4..].try_into().ok()
}
