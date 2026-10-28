//! The Mapping page's state: the selected TCA control and the mapping being built for it, the NEO target inspected
//! with nothing selected, and the correction log. Changes are saved at once, one recorded mapping at a time.
use crate::diagrams::Hand;
use crate::mapping::{self, current, describe, Layer, Mapping, Neo, Slot, HAT_SOURCES};
use crate::worker::Request;
use neo_common::config::{Config, HatMode, Mode};
use neo_common::labels::Entry;

/// The side panel's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Selected,
    Buttons,
    Log,
}

/// What the page is showing: a selected control, an inspected NEO target, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Selected,
    Inspecting,
    Nothing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Map {
    pub selected: Option<String>,
    /// The mapping being built for the selection.
    pub proposal: Option<Mapping>,
    /// The NEO target inspected with nothing selected.
    pub reverse: Option<Neo>,
    pub hand: Hand,
    /// Sources held down right now, in the daemon's order.
    pub held: Vec<String>,
    /// Whether an emulation session is running.
    pub emulating: bool,
    pub labels: Vec<Entry>,
    /// A save is in flight.
    pub busy: bool,
    pub slot: Slot,
    pub layer: Layer,
    pub side: Side,
    /// The last outcome, under the side panel's actions.
    pub note: Option<String>,
    /// The recording in flight: what it maps, and the new mapping as text.
    recording: Option<(String, String)>,
}

impl Default for Map {
    fn default() -> Self {
        Self {
            selected: None,
            proposal: None,
            reverse: None,
            hand: Hand::Right,
            held: Vec::new(),
            emulating: false,
            labels: Vec::new(),
            busy: false,
            slot: Slot::Press,
            layer: Layer::Normal,
            side: Side::Selected,
            note: None,
            recording: None,
        }
    }
}

pub const HELP_SELECTED: &str = "Click NEO buttons or a POV arrow to change this control\u{2019}s mapping, then record it. Changes apply immediately, even while emulating.";
pub const HELP_INSPECTING: &str = "These TCA controls feed it. Select one of them to change its mapping.";
pub const HELP_NOTHING: &str = "Select a TCA control, then click NEO buttons or a POV arrow to build its mapping. With nothing selected, clicking a NEO button shows what feeds it.";

impl Map {
    pub fn focus(&self) -> Focus {
        match (&self.selected, self.reverse) {
            (Some(_), _) => Focus::Selected,
            (None, Some(_)) => Focus::Inspecting,
            (None, None) => Focus::Nothing,
        }
    }

    /// The saved mapping of a source for the action and layer being edited.
    pub fn mapping(&self, saved: &Config, key: &str) -> Mapping {
        current(saved, key, self.slot, self.layer)
    }

    /// Sidestick and quadrant buttons always; hat directions while the hat is mapped.
    pub fn selectable(saved: &Config, key: &str) -> bool {
        if HAT_SOURCES.contains(&key) {
            saved.hat == HatMode::Mapped
        } else {
            mapping::sources().iter().any(|source| source == key)
        }
    }

    pub fn select(&mut self, saved: &Config, key: &str) {
        self.selected = Some(key.to_string());
        self.proposal = Some(self.mapping(saved, key));
        self.reverse = None;
        self.side = Side::Selected;
    }

    pub fn deselect(&mut self) {
        self.selected = None;
        self.proposal = None;
        self.reverse = None;
    }

    /// A NEO target clicked: added to or removed from the proposal, or inspected with nothing selected.
    pub fn pick(&mut self, neo: Neo) {
        match self.proposal.as_mut() {
            Some(proposal) => proposal.toggle(neo),
            None => self.reverse = if self.reverse == Some(neo) { None } else { Some(neo) },
        }
    }

    /// Start again from the saved mapping (after switching action or layer, or Undo changes).
    pub fn revert(&mut self, saved: &Config) {
        self.proposal = self.selected.as_deref().map(|key| current(saved, key, self.slot, self.layer));
    }

    pub fn set_slot(&mut self, saved: &Config, slot: Slot) {
        self.slot = slot;
        self.revert(saved);
    }

    /// The shift layer edits the press only: long press and double tap are the same in both layers.
    pub fn set_layer(&mut self, saved: &Config, layer: Layer) {
        self.layer = layer;
        if layer == Layer::Shift {
            self.slot = Slot::Press;
        }
        self.revert(saved);
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if let Some(proposal) = self.proposal.as_mut() {
            proposal.mode = mode;
        }
    }

    /// Without a shift control there is no shift layer to edit.
    pub fn normalize(&mut self, saved: &Config) {
        if saved.shift.is_none() && self.layer == Layer::Shift {
            self.layer = Layer::Normal;
            self.revert(saved);
        }
    }

    /// The proposal differs from the saved mapping.
    pub fn changed(&self, saved: &Config) -> bool {
        match (&self.selected, &self.proposal) {
            (Some(key), Some(proposal)) => *proposal != self.mapping(saved, key),
            _ => false,
        }
    }

    /// "Sidestick button 2 · long press", "Sidestick button 1 (shift layer)".
    pub fn subject(&self, key: &str) -> String {
        let layer = if self.layer == Layer::Shift { " (shift layer)" } else { "" };
        let slot = if self.slot == Slot::Press { String::new() } else { format!(" \u{b7} {}", self.slot.label().to_lowercase()) };
        format!("{}{layer}{slot}", mapping::label(key))
    }

    /// A mapping as text; an empty shift-layer press says that the normal mapping applies instead.
    pub fn shown(&self, saved: &Config, mapping: &Mapping, slot: Slot) -> String {
        match (&self.selected, self.layer, slot) {
            (Some(key), Layer::Shift, Slot::Press) if mapping.buttons.is_empty() && mapping.pov.is_none() => {
                format!("Normal: {}", describe(&current(saved, key, Slot::Press, Layer::Normal)))
            }
            _ => describe(mapping),
        }
    }

    /// Save the proposal and log it as a correction.
    pub fn record(&mut self, saved: &Config) -> Option<Request> {
        let (Some(key), Some(after)) = (self.selected.clone(), self.proposal.clone()) else { return None };
        let before = self.mapping(saved, &key);
        if self.busy || after == before {
            return None;
        }
        let config = mapping::apply(saved, &key, self.slot, self.layer, &after);
        let what = self.subject(&key);
        let change: String = format!("{} \u{2192} {}", describe(&before), describe(&after)).chars().take(200).collect();
        let mut labels = self.labels.clone();
        labels.push(Entry { label: what.clone(), sources: vec![key], mapping: change });
        self.busy = true;
        self.recording = Some((what, describe(&after)));
        Some(Request::Record { config: Box::new(config), labels })
    }

    /// The hat mode is part of the profile: it is saved at once, like a recorded mapping.
    pub fn set_hat(&mut self, saved: &Config, mode: HatMode) -> Option<Request> {
        if self.busy || saved.hat == mode {
            return None;
        }
        self.busy = true;
        Some(Request::SetHat(Box::new(Config { hat: mode, ..saved.clone() })))
    }

    pub fn forget(&self, index: usize) -> Request {
        Request::Forget(self.labels.iter().enumerate().filter(|(i, _)| *i != index).map(|(_, entry)| entry.clone()).collect())
    }

    /// The daemon's pressed sources: newly pressed selectable ones select themselves while the page is open.
    pub fn press(&mut self, saved: Option<&Config>, pressed: &[String], on_page: bool) {
        let fresh: Vec<&String> = pressed.iter().filter(|key| !self.held.contains(*key)).collect();
        self.held = pressed.to_vec();
        let picked = saved.and_then(|saved| fresh.into_iter().find(|key| Self::selectable(saved, key)).map(|key| (saved, key)));
        if let Some((saved, key)) = picked.filter(|_| on_page && !self.busy) {
            self.select(saved, key);
        }
    }

    pub fn recorded(&mut self, log: Result<Vec<Entry>, String>, running: bool) {
        let (what, after) = self.recording.take().unwrap_or_default();
        let applied = if running { " Applied to the running emulation." } else { "" };
        self.busy = false;
        self.note = Some(match log {
            Ok(labels) => {
                self.labels = labels;
                format!("Recorded {what}: {after}.{applied}")
            }
            Err(error) => format!("Mapping saved, but the correction log was not updated: {error}.{applied}"),
        });
    }

    pub fn hat_set(&mut self, hat: HatMode, running: bool) {
        let applied = if running { " Applied to the running emulation." } else { "" };
        self.busy = false;
        self.note = Some(match hat {
            HatMode::Mapped => format!("The hat now sends its mapped controls; the camera mini-stick gets no input.{applied}"),
            HatMode::Camera => {
                if self.selected.as_deref().is_some_and(|key| key.starts_with("hat-")) {
                    self.deselect();
                }
                format!("The hat drives the camera again; hat mappings are kept but inactive.{applied}")
            }
        });
    }

    pub fn not_saved(&mut self, message: &str) {
        self.busy = false;
        self.recording = None;
        self.note = Some(format!("Not saved: {message}"));
    }

    pub fn forgot(&mut self, result: Result<Vec<Entry>, String>) {
        self.note = Some(match result {
            Ok(labels) => {
                self.labels = labels;
                "Removed from the correction log. The mapping itself is unchanged.".into()
            }
            Err(error) => format!("Not removed: {error}"),
        });
    }

    /// The text above the NEO diagram.
    pub fn context(&self, saved: &Config) -> String {
        match (&self.selected, &self.proposal, self.reverse) {
            (Some(key), Some(proposal), _) => format!("Mapping of {} \u{2192} {}", self.subject(key), describe(proposal)),
            (_, _, Some(neo)) => {
                let feeds = mapping::sources_for(saved, neo);
                let fed = if feeds.is_empty() { "nothing".to_string() } else { feeds.iter().map(|key| mapping::short(key)).collect::<Vec<_>>().join(" \u{b7} ") };
                format!("Inspecting {} \u{b7} fed by {fed}", mapping::neo_name(neo))
            }
            _ => "Select a TCA control to light up its NEO targets".into(),
        }
    }

    /// The live well under the NEO diagram.
    pub fn sending(&self) -> String {
        if !self.held.is_empty() {
            format!("Now sending: {}", self.held.iter().map(|key| mapping::label(key)).collect::<Vec<_>>().join(", "))
        } else if self.emulating {
            "Nothing pressed".into()
        } else {
            "Start emulation to see presses.".into()
        }
    }
}
