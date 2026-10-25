//! Printed-label recordings made in Identify mode: which HID source each printed control sends.
use crate::{config, maths};
use serde::{Deserialize, Serialize};

pub const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Text printed on or beside the control, as typed by the user.
    pub label: String,
    /// Every source held when the press was detected ("s12", "q3", "hat-up").
    pub sources: Vec<String>,
    /// The mapping shown at the time, for review.
    pub mapping: String,
}

pub fn valid_source(key: &str) -> bool {
    config::source(key).is_some() || key.strip_prefix("hat-").is_some_and(|d| maths::HAT_NAMES.contains(&d))
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let entries: Vec<Entry> = serde_json::from_str(text).map_err(|e| format!("Invalid recordings: {e}."))?;
    if entries.len() > MAX_ENTRIES {
        return Err(format!("At most {MAX_ENTRIES} recordings."));
    }
    for entry in &entries {
        if entry.label.trim().is_empty() || entry.label.chars().count() > 60 {
            return Err("Each printed label needs 1 to 60 characters.".into());
        }
        if entry.sources.is_empty() || entry.sources.len() > 8 || !entry.sources.iter().all(|s| valid_source(s)) {
            return Err("Each recording needs the detected TCA controls.".into());
        }
        if entry.mapping.chars().count() > 200 {
            return Err("Mapping description is too long.".into());
        }
    }
    Ok(entries)
}
