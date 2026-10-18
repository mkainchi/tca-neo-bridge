//! State shared by the tray, the emulation loop and the configuration page.
use crate::config::{self, Config};
use crate::hid::{self, HidApi};
use crate::models::DeviceInfo;
use std::collections::BTreeMap;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Idle,
    Starting,
    Emulating,
    Stopping,
    Error(String),
}

impl Status {
    pub fn label(&self) -> String {
        match self {
            Status::Idle => "Idle".into(),
            Status::Starting => "Starting".into(),
            Status::Emulating => "Emulating".into(),
            Status::Stopping => "Stopping".into(),
            Status::Error(message) => format!("Error: {message}"),
        }
    }

    pub fn running(&self) -> bool {
        matches!(self, Status::Starting | Status::Emulating | Status::Stopping)
    }
}

/// What the running session holds, for the page's status pills; None while idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Links {
    /// "hotas" or "validation".
    pub source: &'static str,
    /// HidHide hides the controllers from other applications.
    pub hidden: bool,
    /// Controllers open for reading.
    pub inputs: usize,
    /// The virtual NEO software device exists.
    pub virtual_neo: bool,
}

pub struct Shared {
    pub config_path: PathBuf,
    /// Generation counter lets the emulation loop hot-reload without locking every frame.
    pub config: RwLock<(u64, Config)>,
    pub pressed: Mutex<Vec<String>>,
    /// Latest travel of the split High-G levers while emulating.
    pub levers: Mutex<Option<[f64; 2]>>,
    /// Latest axis positions (-1..1) per device while emulating, for axis detection on the page.
    pub axes: Mutex<BTreeMap<String, Vec<f64>>>,
    /// The session's controllers while emulating from them.
    pub devices: Mutex<Option<Vec<DeviceInfo>>>,
    /// Lists present controllers while idle (the configuration page); None in tests that need no hardware.
    pub hid: Option<HidApi>,
    pub status: Mutex<Status>,
    pub links: Mutex<Option<Links>>,
    /// Private diagnostics (status changes, panics). Never contains device paths or serials.
    pub log: Option<PathBuf>,
}

/// Append one timestamped line; diagnostics must never break emulation, so write failures are ignored.
pub fn append(path: &std::path::Path, text: &str) {
    use std::io::Write;
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(path).and_then(|mut file| writeln!(file, "{millis} {text}"));
}

impl Shared {
    pub fn new(config_path: PathBuf, config: Config) -> Self {
        Self {
            config_path,
            config: RwLock::new((0, config)),
            pressed: Mutex::new(Vec::new()),
            levers: Mutex::new(None),
            axes: Mutex::new(BTreeMap::new()),
            devices: Mutex::new(None),
            hid: None,
            status: Mutex::new(Status::Idle),
            links: Mutex::new(None),
            log: None,
        }
    }

    pub fn with_hid(mut self, hid: HidApi) -> Self {
        self.hid = Some(hid);
        self
    }

    /// The session's controllers while emulating from them, otherwise the controllers present now.
    pub fn present(&self) -> Result<Vec<DeviceInfo>, String> {
        if let Some(devices) = self.devices.lock().unwrap().clone() {
            return Ok(devices);
        }
        self.hid.map_or(Ok(Vec::new()), |api| hid::list(&api))
    }

    pub fn with_log(mut self, path: PathBuf) -> Self {
        self.log = Some(path);
        self
    }

    /// Identify-mode recordings live next to the profile.
    pub fn labels_path(&self) -> PathBuf {
        self.config_path.with_file_name("label-map.json")
    }

    pub fn config(&self) -> (u64, Config) {
        self.config.read().unwrap().clone()
    }

    pub fn generation(&self) -> u64 {
        self.config.read().unwrap().0
    }

    /// Validate, persist, then publish to the running emulation.
    pub fn save(&self, config: Config) -> Result<(), String> {
        config::save(&self.config_path, &config)?;
        let mut guard = self.config.write().unwrap();
        *guard = (guard.0 + 1, config);
        Ok(())
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn set_status(&self, status: Status) {
        if let Some(path) = &self.log {
            append(path, &format!("status {}", status.label()));
        }
        *self.status.lock().unwrap() = status;
    }
}
