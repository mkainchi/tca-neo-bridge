//! State shared by the tray, the emulation loop and the configuration page.
use crate::config::{self, Config};
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
    /// "tca" or "validation".
    pub source: &'static str,
    /// HidHide hides the TCA from other applications.
    pub hidden: bool,
    /// Both TCA devices are open.
    pub tca: bool,
    /// The virtual NEO software device exists.
    pub virtual_neo: bool,
}

pub struct Shared {
    pub config_path: PathBuf,
    /// Generation counter lets the emulation loop hot-reload without locking every frame.
    pub config: RwLock<(u64, Config)>,
    pub pressed: Mutex<Vec<String>>,
    /// Latest quadrant lever positions (left, right) while emulating from the TCA.
    pub levers: Mutex<Option<[f64; 2]>>,
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
        Self { config_path, config: RwLock::new((0, config)), pressed: Mutex::new(Vec::new()), levers: Mutex::new(None), status: Mutex::new(Status::Idle), links: Mutex::new(None), log: None }
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
