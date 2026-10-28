//! What the daemon's HTTP API sends: the live monitor it fills, and the replies the configuration app reads.
use crate::config::{Config, Profile};
use crate::models::DeviceInfo;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Raw inputs next to what the NEO receives, for the live previews. Flight inputs are -1..1 as bound (before
/// inversion); throttle travel and the split levers run from 0 at idle to 1 fully forward.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Monitor {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    /// Travel of the throttle input in control (the forward lever while split); None without a throttle axis.
    pub throttle: Option<f64>,
    /// The camera lever's position, -1..1 after its inversion.
    pub camera_lever: Option<f64>,
    pub levers: Option<[f64; 2]>,
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
    /// "hotas" or "validation".
    pub source: String,
    /// HidHide hides the controllers from other applications.
    pub hidden: bool,
    /// Controllers open for reading.
    pub inputs: usize,
    pub virtual_neo: bool,
}

/// `GET /api/live`, polled while the app is visible.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Live {
    /// Device-qualified controls held now ("t16000m/b3", "tca-stick/hat-up").
    pub pressed: Vec<String>,
    /// Travel of the split High-G lever pair.
    pub levers: Option<[f64; 2]>,
    /// Every axis of every open controller, -1..1 in layout order; only while emulating (axis detection reads it).
    #[serde(default)]
    pub axes: BTreeMap<String, Vec<f64>>,
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

/// `GET /api/devices`: the controllers present, those taking part, their set and its mapping.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Devices {
    pub devices: Vec<DeviceInfo>,
    pub active: Vec<String>,
    /// The sorted ids of the active controllers joined by "+".
    pub setup: String,
    /// The saved mapping for this set, or its default.
    pub profile: Profile,
    pub default: Profile,
}