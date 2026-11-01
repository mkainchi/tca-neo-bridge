//! Printed-label recordings made in Identify mode: the shared format (`neo_common::labels`) plus persistence.
pub use neo_common::labels::*;
use std::path::Path;

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
