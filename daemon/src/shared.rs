//! State shared by the tray, the emulation loop and the configuration API.
use crate::config::{self, Config};
use crate::driver::Notice;
use crate::profiles;
use crate::neo::Monitor;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// Generation counter lets the emulation loop hot-reload without locking every frame. It also changes when a
    /// preview starts or ends.
    pub config: RwLock<(u64, Config)>,
    /// Unsaved settings the running emulation uses instead of the saved profile ("Try in game"); never persisted.
    pub preview: Mutex<Option<Config>>,
    /// Pause input: the emulation sends a neutral NEO while set.
    pub paused: AtomicBool,
    /// Latest raw inputs and NEO axes while emulating from the TCA.
    pub monitor: Mutex<Option<Monitor>>,
    pub pressed: Mutex<Vec<String>>,
    /// Latest quadrant lever positions (left, right) while emulating from the TCA.
    pub levers: Mutex<Option<[f64; 2]>>,
    pub status: Mutex<Status>,
    pub links: Mutex<Option<Links>>,
    /// Launch-order warnings from the last start (Steam running, Steam Input on).
    pub warnings: Mutex<Vec<String>>,
    /// Tray balloons waiting to be shown (profile switches from the TCA, for example).
    pub notices: Mutex<VecDeque<Notice>>,
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
            preview: Mutex::new(None),
            paused: AtomicBool::new(false),
            monitor: Mutex::new(None),
            pressed: Mutex::new(Vec::new()),
            levers: Mutex::new(None),
            status: Mutex::new(Status::Idle),
            links: Mutex::new(None),
            notices: Mutex::new(VecDeque::new()),
            warnings: Mutex::new(Vec::new()),
            log: None,
        }
    }

    pub fn with_log(mut self, path: PathBuf) -> Self {
        self.log = Some(path);
        self
    }

    /// Named profile snapshots live next to the active profile.
    pub fn profiles_dir(&self) -> PathBuf {
        self.config_path.with_file_name("profiles")
    }

    /// Make a saved profile the active one: it replaces config.json (ending any preview) and is remembered.
    pub fn activate(&self, name: &str) -> Result<Config, String> {
        let profile = profiles::load(&self.profiles_dir(), name)?;
        self.save(profile.clone())?;
        profiles::set_active(&self.profiles_dir(), Some(name))?;
        Ok(profile)
    }

    pub fn notice(&self, notice: Notice) {
        self.notices.lock().unwrap().push_back(notice);
    }

    pub fn take_notice(&self) -> Option<Notice> {
        self.notices.lock().unwrap().pop_front()
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

    /// What the emulation runs: the preview while one is set, else the saved profile.
    pub fn active(&self) -> (u64, Config) {
        let (generation, saved) = self.config();
        (generation, self.preview.lock().unwrap().clone().unwrap_or(saved))
    }

    fn bump(&self) {
        self.config.write().unwrap().0 += 1;
    }

    /// Validate, persist, then publish to the running emulation; a save ends any preview.
    pub fn save(&self, config: Config) -> Result<(), String> {
        config::save(&self.config_path, &config)?;
        *self.preview.lock().unwrap() = None;
        let mut guard = self.config.write().unwrap();
        *guard = (guard.0 + 1, config);
        Ok(())
    }

    /// Start (validated, not saved) or end a preview.
    pub fn set_preview(&self, preview: Option<Config>) -> Result<(), String> {
        preview.as_ref().map_or(Ok(()), config::validate)?;
        *self.preview.lock().unwrap() = preview;
        self.bump();
        Ok(())
    }

    pub fn previewing(&self) -> bool {
        self.preview.lock().unwrap().is_some()
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
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
