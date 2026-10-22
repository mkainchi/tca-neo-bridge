//! Named profile snapshots in `<data>\profiles\<name>.json`, beside the active `config.json`.
use crate::config::{self, Config};
use std::path::{Path, PathBuf};

/// Letters, digits, spaces, `_` and `-`, 1 to 40 characters, not starting or ending with a space. This also keeps
/// names from reaching outside the profiles folder.
pub fn valid(name: &str) -> bool {
    (1..=40).contains(&name.chars().count())
        && name.trim() == name
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
}

fn checked(name: &str) -> Result<&str, String> {
    if valid(name) {
        Ok(name)
    } else {
        Err("Profile names use 1 to 40 letters, digits, spaces, _ or -.".into())
    }
}

fn file(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

/// Saved profile names, sorted case-insensitively. A missing folder means none.
pub fn list(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.strip_suffix(".json").map(str::to_string))
        .filter(|name| valid(name))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names
}

/// Store a profile under a name (validated, atomically), replacing one of the same name.
pub fn save(dir: &Path, name: &str, profile: &Config) -> Result<(), String> {
    let name = checked(name)?;
    std::fs::create_dir_all(dir).map_err(|_| "The profiles folder could not be created.".to_string())?;
    config::save(&file(dir, name), profile)
}

pub fn load(dir: &Path, name: &str) -> Result<Config, String> {
    let name = checked(name)?;
    let text = std::fs::read_to_string(file(dir, name)).map_err(|_| format!("There is no profile named {name}."))?;
    config::parse(&text)
}

pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
    let name = checked(name)?;
    std::fs::remove_file(file(dir, name)).map_err(|_| format!("There is no profile named {name}."))
}

/// The profile after `current` in name order, wrapping round; the first one when `current` is not saved.
pub fn next(dir: &Path, current: Option<&str>) -> Option<String> {
    let names = list(dir);
    let after = current.and_then(|current| names.iter().position(|name| name == current)).map_or(0, |index| index + 1);
    names.get(after % names.len().max(1)).cloned()
}

/// The name of the profile last activated, remembered across restarts.
pub fn active(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("active.txt")).ok().map(|text| text.trim().to_string()).filter(|name| valid(name))
}

pub fn set_active(dir: &Path, name: Option<&str>) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .and_then(|()| std::fs::write(dir.join("active.txt"), name.unwrap_or("")))
        .map_err(|_| "The active profile could not be recorded.".to_string())
}
