//! What the window shows, kept apart from drawing so it can be tested on its own.
use crate::worker::Reply;
use neo_common::api::{Links, Monitor, Profiles};
use neo_common::config::Config;

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

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub page: Page,
    /// The daemon's status text ("Idle", "Emulating", "Error: …").
    pub status: String,
    pub links: Option<Links>,
    pub paused: bool,
    /// A draft runs in game instead of the saved profile.
    pub previewing: bool,
    pub warnings: Vec<String>,
    /// The warnings last dismissed; the banner returns when they change.
    pub dismissed: Vec<String>,
    /// Live input while emulating from the TCA.
    pub monitor: Option<Monitor>,
    pub saved: Option<Config>,
    pub defaults: Option<Config>,
    /// Where the saved profile lives.
    pub profile_path: String,
    pub profiles: Profiles,
    /// The last failure, shown in the save bar.
    pub note: Option<String>,
    /// The daemon has gone or no longer knows this app: the window closes.
    pub closing: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            page: Page::Mapping,
            status: "Loading\u{2026}".into(),
            links: None,
            paused: false,
            previewing: false,
            warnings: Vec::new(),
            dismissed: Vec::new(),
            monitor: None,
            saved: None,
            defaults: None,
            profile_path: "config.json".into(),
            profiles: Profiles { profiles: Vec::new(), active: None },
            note: None,
            closing: false,
        }
    }
}

impl State {
    pub fn apply(&mut self, reply: Reply) {
        match reply {
            Reply::Loaded(config, profiles) => {
                let config = *config;
                self.saved = Some(config.config);
                self.defaults = Some(config.defaults);
                self.profile_path = config.profile;
                self.status = config.status;
                self.links = config.links;
                self.previewing = config.preview.is_some();
                self.warnings = config.warnings;
                self.profiles = profiles;
                self.note = None;
            }
            Reply::Live(live) => {
                let live = *live;
                self.status = live.status;
                self.links = live.links;
                self.paused = live.paused;
                self.previewing = live.preview;
                self.warnings = live.warnings;
                self.monitor = live.monitor;
            }
            Reply::Failed(message) => self.note = Some(message),
            Reply::Fatal(_) => self.closing = true,
        }
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
