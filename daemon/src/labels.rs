//! Printed-label recordings made in Identify mode: which HID source each printed control sends.
use crate::{config, models, neo};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Text printed on or beside the control, as typed by the user.
    pub label: String,
    /// Every source held when the press was detected ("tca-stick/b12", "t16000m/hat-up").
    pub sources: Vec<String>,
    /// The mapping shown at the time, for review.
    pub mapping: String,
}

pub fn valid_source(key: &str) -> bool {
    config::source(key).is_some()
        || key.rsplit_once('/').is_some_and(|(device, control)| models::valid_id(device) && control.strip_prefix("hat-").is_some_and(|d| neo::HAT_NAMES.contains(&d)))
}

/// Recordings of the TCA-only bridge used "s12", "q3" and "hat-up-right"; they name the TCA devices now.
pub fn migrate_source(key: &str) -> String {
    let hat = key.strip_prefix("hat-").filter(|d| neo::HAT_NAMES.contains(d)).map(|_| format!("{}/{key}", models::TCA_STICK));
    hat.or_else(|| config::migrate_key(key)).unwrap_or_else(|| key.to_string())
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let mut entries: Vec<Entry> = serde_json::from_str(text).map_err(|e| format!("Invalid recordings: {e}."))?;
    for entry in &mut entries {
        entry.sources = entry.sources.iter().map(|s| migrate_source(s)).collect();
    }
    if entries.len() > MAX_ENTRIES {
        return Err(format!("At most {MAX_ENTRIES} recordings."));
    }
    for entry in &entries {
        if entry.label.trim().is_empty() || entry.label.chars().count() > 60 {
            return Err("Each printed label needs 1 to 60 characters.".into());
        }
        if entry.sources.is_empty() || entry.sources.len() > 8 || !entry.sources.iter().all(|s| valid_source(s)) {
            return Err("Each recording needs the detected controls.".into());
        }
        if entry.mapping.chars().count() > 200 {
            return Err("Mapping description is too long.".into());
        }
    }
    Ok(entries)
}

pub fn load(path: &Path) -> Result<Vec<Entry>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err("The recordings could not be read.".into()),
    }
}

pub fn save(path: &Path, entries: &[Entry]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_string_pretty(entries).expect("recordings serialize"))
        .and_then(|()| std::fs::rename(&temporary, path))
        .map_err(|_| "The recordings could not be saved.".to_string())
}
