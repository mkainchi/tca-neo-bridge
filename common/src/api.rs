//! What the daemon's HTTP API sends: the live monitor it fills, and the replies the configuration app reads.
use crate::config::Config;
use serde::{Deserialize, Serialize};

/// Raw inputs (axes -1..1, lever travel 0..1) next to what the NEO receives, for the live previews.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Monitor {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub slider: f64,
    pub levers: [f64; 2],
    pub axes: [u16; 7],
    pub armed: bool,
    pub precision: bool,
    /// The shift control is held.
    pub shift: bool,
    /// NEO buttons 1..15 as bits 0..14, and the POV (8 when centred), as sent.
    pub buttons: u16,
    pub pov: u8,
    /// The head-tracking sample (degrees) the camera used this frame.
    pub head: Option<[f64; 2]>,
}

/// What the running session holds, as the status pills read it; None while idle.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Links {
    /// "tca" or "validation".
    pub source: String,
    pub hidden: bool,
    pub tca: bool,
    pub virtual_neo: bool,
}

/// `GET /api/live`, polled while the app is visible.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Live {
    pub pressed: Vec<String>,
    pub levers: Option<[f64; 2]>,
    pub monitor: Option<Monitor>,
    pub paused: bool,
    pub preview: bool,
    pub warnings: Vec<String>,
    pub status: String,
    pub links: Option<Links>,
}

/// `GET /api/config`: the saved profile, the defaults and the daemon's state when the app opens.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ConfigReply {
    pub config: Config,
    pub defaults: Config,
    pub status: String,
    pub links: Option<Links>,
    /// Where the saved profile lives.
    pub profile: String,
    /// The draft running in game, while a preview is on.
    pub preview: Option<Config>,
    pub warnings: Vec<String>,
}

/// `GET /api/profiles` and the reply to every profile action.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Profiles {
    pub profiles: Vec<String>,
    pub active: Option<String>,
}
