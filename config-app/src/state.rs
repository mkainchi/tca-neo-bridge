//! What the window shows, kept apart from drawing so it can be tested on its own.
use crate::map::Map;
use crate::settings::Settings;
use crate::worker::{Loaded, Reply, Request};
use neo_common::api::{Links, Monitor, Profiles};
use neo_common::config::{Config, HatMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Mapping,
    Axes,
    Throttle,
    Camera,
    Helpers,
    Live,
}

impl Page {
    /// Rail order; a separator follows Mapping and Buttons & helpers.
    pub const ALL: [Page; 6] = [Page::Mapping, Page::Axes, Page::Throttle, Page::Camera, Page::Helpers, Page::Live];

    pub fn title(self) -> &'static str {
        match self {
            Page::Mapping => "Mapping",
            Page::Axes => "Flight axes",
            Page::Throttle => "Throttle",
            Page::Camera => "Camera & head",
            Page::Helpers => "Buttons & helpers",
            Page::Live => "Live",
        }
    }

    /// The pages that edit the settings draft and show the save bar's actions.
    pub fn settings(self) -> bool {
        matches!(self, Page::Axes | Page::Throttle | Page::Camera | Page::Helpers)
    }

    pub fn help(self) -> &'static str {
        match self {
            Page::Mapping => "Press a TCA control to select it; its NEO targets light up as you press.",
            Page::Axes => "Move the stick: the dot and every meter follow it through your unsaved settings.",
            Page::Throttle => "Move the levers: the needle, zone and NEO throttle follow them live.",
            Page::Camera => "Press the hat or move your head: the hold and camera output show live.",
            Page::Helpers => "Hold a helper or tap a button: states and the press timeline update as you do.",
            Page::Live => "Everything the TCA sends and what the NEO receives, right now.",
        }
    }
}

/// A status pill's colour: off (grey), ok (green), busy (amber) or fault (red).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Off,
    Ok,
    Busy,
    Fault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pills {
    pub status: Tone,
    pub tca: (Tone, &'static str),
    pub hidhide: (Tone, &'static str),
    pub neo: (Tone, &'static str),
}

/// The header pills for the daemon's status text and what its session holds.
pub fn pills(status: &str, links: Option<&Links>) -> Pills {
    let tone = if status.starts_with("Emulating") {
        Tone::Ok
    } else if status.starts_with("Starting") || status.starts_with("Stopping") {
        Tone::Busy
    } else if ["Error", "unreachable", "unavailable"].iter().any(|prefix| status.starts_with(prefix)) {
        Tone::Fault
    } else {
        Tone::Off
    };
    let validation = links.is_some_and(|links| links.source == "validation");
    Pills {
        status: tone,
        tca: match links {
            _ if validation => (Tone::Busy, "Source: validation stream"),
            Some(links) if links.tca => (Tone::Ok, "TCA: sidestick + quadrant open"),
            Some(_) => (Tone::Busy, "TCA: reconnecting\u{2026}"),
            None => (Tone::Off, "TCA: not open"),
        },
        hidhide: match links {
            Some(links) if links.hidden => (Tone::Ok, "HidHide: TCA hidden"),
            _ if validation => (Tone::Off, "HidHide: not needed"),
            _ => (Tone::Off, "HidHide: off"),
        },
        neo: match links {
            Some(links) if links.virtual_neo => (Tone::Ok, "Virtual NEO present"),
            _ => (Tone::Off, "Virtual NEO: absent"),
        },
    }
}

/// The header's profile menu: open or not, the name being typed for Save as, its last outcome, and whether Delete is
/// waiting for its second click.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileMenu {
    pub open: bool,
    pub name: String,
    pub invalid: bool,
    /// (text, is a problem).
    pub note: Option<(String, bool)>,
    pub confirm_delete: bool,
    /// A file action asked for this frame, carried out after drawing (the dialogs are modal).
    pub want: Option<Want>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Import,
    Export,
    Print,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub page: Page,
    /// The daemon's status text ("Idle", "Emulating", "Error: …").
    pub status: String,
    pub links: Option<Links>,
    pub paused: bool,
    pub warnings: Vec<String>,
    /// The warnings last dismissed; the banner returns when they change.
    pub dismissed: Vec<String>,
    /// Live input while emulating from the TCA.
    pub monitor: Option<Monitor>,
    /// The quadrant levers (left, right) while emulating from the TCA.
    pub levers: Option<[f64; 2]>,
    pub saved: Option<Config>,
    pub defaults: Option<Config>,
    /// Where the saved profile lives.
    pub profile_path: String,
    pub profiles: Profiles,
    /// The last failure, shown in the save bar.
    pub note: Option<String>,
    /// The daemon has gone or no longer knows this app: the window closes.
    pub closing: bool,
    pub map: Map,
    pub settings: Settings,
    pub menu: ProfileMenu,
    /// The window's clock in milliseconds (the press timeline and the hat hold).
    pub now: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            page: Page::Mapping,
            status: "Loading\u{2026}".into(),
            links: None,
            paused: false,
            warnings: Vec::new(),
            dismissed: Vec::new(),
            monitor: None,
            levers: None,
            saved: None,
            defaults: None,
            profile_path: "config.json".into(),
            profiles: Profiles { profiles: Vec::new(), active: None },
            note: None,
            closing: false,
            map: Map::default(),
            settings: Settings::default(),
            menu: ProfileMenu::default(),
            now: 0.0,
        }
    }
}

impl State {
    /// Take in a reply from the daemon; `now` is the window's clock in milliseconds. Returns the requests that
    /// follow from it (the next queued preview).
    pub fn apply(&mut self, reply: Reply, now: f64) -> Vec<Request> {
        self.now = now;
        match reply {
            Reply::Loaded(loaded) => {
                let Loaded { config, profiles, labels } = *loaded;
                self.defaults = Some(config.defaults);
                self.profile_path = config.profile;
                self.status = config.status;
                self.map.emulating = config.links.is_some();
                self.links = config.links;
                self.warnings = config.warnings;
                self.note = profiles.as_ref().err().map(|error| format!("Saved profiles could not be listed: {error}"));
                self.profiles = profiles.unwrap_or(Profiles { profiles: Vec::new(), active: None });
                match labels {
                    Ok(labels) => self.map.labels = labels,
                    Err(error) => self.map.note = Some(format!("Earlier corrections could not be loaded: {error}")),
                }
                // A preview already running is the draft to keep editing.
                self.settings.show(config.preview.clone().unwrap_or_else(|| config.config.clone()));
                if config.preview.is_some() {
                    self.settings.resume_preview();
                }
                self.saved(config.config);
            }
            Reply::Live(live) => {
                let live = *live;
                self.status = live.status;
                self.map.emulating = live.links.is_some();
                self.links = live.links;
                self.paused = live.paused;
                self.warnings = live.warnings;
                self.monitor = live.monitor;
                self.levers = live.levers;
                if !live.preview {
                    self.settings.preview_gone();
                }
                let fresh: Vec<String> = live.pressed.iter().filter(|key| !self.map.held.contains(key)).cloned().collect();
                let hat_camera = self.saved.as_ref().is_some_and(|saved| saved.hat == HatMode::Camera);
                self.map.press(self.saved.as_ref(), &live.pressed, self.page == Page::Mapping);
                return self.settings.press(&live.pressed, &fresh, hat_camera, now);
            }
            Reply::Recorded { config, running, log } => {
                self.mappings(&config);
                self.saved(*config);
                self.map.recorded(log, running);
            }
            Reply::HatSet { config, running } => {
                let hat = config.hat;
                if let Some(draft) = self.settings.draft.as_mut() {
                    draft.hat = hat;
                }
                self.saved(*config);
                self.map.hat_set(hat, running);
            }
            Reply::NotSaved(message) => self.map.not_saved(&message),
            Reply::Forgot(result) => self.map.forgot(result),
            Reply::Saved { config, running } => {
                self.saved(*config);
                self.settings.saved(running);
            }
            Reply::SaveFailed(message) => self.settings.save_failed(&message),
            Reply::Previewed => return self.settings.previewed(),
            Reply::PreviewFailed(message) => self.settings.preview_failed(&message),
            Reply::PreviewEnded => {}
            Reply::PreviewEndFailed(message) => self.settings.preview_end_failed(&message),
            Reply::Adopted { config, profiles, text } => {
                self.settings.adopt((*config).clone(), &text);
                self.saved(*config);
                if let Some(profiles) = profiles {
                    self.profiles = profiles;
                }
                self.menu.note = Some((text, false));
            }
            Reply::Listed { profiles, text } => {
                self.profiles = profiles;
                self.menu.name.clear();
                self.menu.note = Some((text, false));
            }
            Reply::ProfileFailed(text) => self.menu.note = Some((text, true)),
            Reply::Failed(message) => self.note = Some(message),
            Reply::Fatal(_) => self.closing = true,
        }
        Vec::new()
    }

    /// A recorded mapping is saved at once: the settings draft takes it too.
    fn mappings(&mut self, config: &Config) {
        if let Some(draft) = self.settings.draft.as_mut() {
            draft.buttons = config.buttons.clone();
            draft.pov = config.pov.clone();
            draft.behaviours = config.behaviours.clone();
            draft.shifted = config.shifted.clone();
            draft.long_press = config.long_press.clone();
            draft.double_tap = config.double_tap.clone();
        }
    }

    /// The saved profile changed: the mapping page follows it.
    fn saved(&mut self, config: Config) {
        self.map.normalize(&config);
        self.saved = Some(config);
    }

    /// Show a rail page. Leaving the settings pages ends a preview, keeping the draft.
    pub fn show(&mut self, page: Page) -> Vec<Request> {
        self.page = page;
        if page.settings() {
            Vec::new()
        } else {
            self.settings.leave()
        }
    }

    /// Switching or importing replaces the saved settings: unsaved edits must be saved or discarded first.
    fn unsaved(&mut self) -> bool {
        if self.settings.dirty {
            self.menu.note = Some(("Save or discard unsaved settings first.".into(), true));
        }
        self.settings.dirty
    }

    pub fn activate(&mut self, name: &str) -> Vec<Request> {
        if self.unsaved() {
            return Vec::new();
        }
        vec![Request::Activate(name.into())]
    }

    /// Save the saved settings under the typed name (or replace a profile of that name), and make it active.
    pub fn save_as(&mut self) -> Vec<Request> {
        let name = self.menu.name.trim().to_string();
        self.menu.invalid = !neo_common::profiles::valid(&name);
        if self.menu.invalid {
            self.menu.note = Some(("Use 1 to 40 letters, digits, spaces, _ or -.".into(), true));
            return Vec::new();
        }
        if self.unsaved() {
            return Vec::new();
        }
        vec![Request::SaveAs(name)]
    }

    /// Delete the active profile: the first click asks, the second deletes. The settings themselves stay.
    pub fn delete(&mut self) -> Vec<Request> {
        let Some(name) = self.profiles.active.clone() else { return Vec::new() };
        self.menu.confirm_delete = !self.menu.confirm_delete;
        if self.menu.confirm_delete {
            return Vec::new();
        }
        vec![Request::DeleteProfile(name)]
    }

    /// A profile file's text: the daemon checks and upgrades it before it becomes the saved settings.
    pub fn import(&mut self, text: &str, name: &str) -> Vec<Request> {
        if self.unsaved() {
            return Vec::new();
        }
        match serde_json::from_str(text) {
            Ok(profile) => vec![Request::Import { profile, name: name.into() }],
            Err(error) => {
                self.menu.note = Some((format!("Not imported: Invalid profile: {error}."), true));
                Vec::new()
            }
        }
    }

    /// The saved settings as a JSON file to keep or share; a file that could not be written is reported.
    pub fn export(&mut self, path: &std::path::Path) {
        let Some(saved) = &self.saved else { return };
        let name = path.file_name().map_or(String::new(), |name| name.to_string_lossy().into_owned());
        self.menu.note = Some(match std::fs::write(path, format!("{}\n", neo_common::config::to_json(saved))) {
            Ok(()) => (format!("Exported {name}."), false),
            Err(_) => (format!("Not exported: {name} could not be written."), true),
        });
    }

    /// The file name an export suggests: the active profile's, or the bridge's.
    pub fn export_name(&self) -> String {
        format!("{}.json", self.profiles.active.as_deref().unwrap_or("tca-neo-bridge"))
    }

    /// The draft runs in game instead of the saved profile.
    pub fn previewing(&self) -> bool {
        self.settings.previewing
    }
    /// Previews follow the device while emulating from the TCA.
    pub fn live(&self) -> bool {
        self.monitor.is_some()
    }

    pub fn show_warnings(&self) -> bool {
        !self.warnings.is_empty() && self.dismissed != self.warnings
    }

    pub fn profile_name(&self) -> &str {
        self.profiles.active.as_deref().unwrap_or("Unsaved profile")
    }
}
