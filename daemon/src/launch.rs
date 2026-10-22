//! Launch-order checks, warn only: Steam already running when emulation starts, and Steam Input left on for
//! ACE COMBAT 8. Nothing is started, stopped or changed.
use crate::process::Runner;
use std::path::{Path, PathBuf};

/// ACE COMBAT 8's Steam app id.
pub const AC8: &str = "2288340";

pub const STEAM_RUNNING: &str = "Steam was already running when emulation started, so it may still see your HOTAS instead of the NEO. Restart Steam, then the game.";
pub const STEAM_INPUT: &str = "Steam Input is on for ACE COMBAT 8, so Steam remaps the NEO. In Steam, open the game's Properties › Controller and choose Disable Steam Input.";

fn run(runner: &Runner, tool: &Path, args: &[&str]) -> Option<String> {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    match runner(tool, &args) {
        Ok((Some(0), stdout)) => Some(stdout),
        _ => None,
    }
}

/// `tasklist` lists steam.exe (CSV, no header) when Steam runs; anything unexpected counts as not running.
pub fn steam_running(runner: &Runner, system: &Path) -> bool {
    run(runner, &system.join("tasklist.exe"), &["/FI", "IMAGENAME eq steam.exe", "/FO", "CSV", "/NH"]).is_some_and(|out| out.to_lowercase().contains("\"steam.exe\""))
}

/// Steam's folder from `reg query HKCU\Software\Valve\Steam /v SteamPath`.
pub fn steam_path(runner: &Runner, system: &Path) -> Option<PathBuf> {
    let out = run(runner, &system.join("reg.exe"), &["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])?;
    let line = out.lines().find(|line| line.trim_start().starts_with("SteamPath"))?;
    let path = line.split_once("REG_SZ")?.1.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Quoted strings and braces of a Valve KeyValues (VDF) text.
fn tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => tokens.push(c.to_string()),
            '"' => tokens.push(chars.by_ref().take_while(|&c| c != '"').collect()),
            _ => {}
        }
    }
    tokens
}

/// The value of `key` directly inside the block named `block`, anywhere in the text.
pub fn vdf_value(text: &str, block: &str, key: &str) -> Option<String> {
    let tokens = tokens(text);
    let start = tokens.windows(2).position(|pair| pair[0] == block && pair[1] == "{")? + 2;
    let mut depth = 0usize;
    let mut index = start;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "{" => depth += 1,
            "}" if depth == 0 => return None,
            "}" => depth -= 1,
            name if depth == 0 && name == key => return tokens.get(index + 1).filter(|value| *value != "{").cloned(),
            _ => {}
        }
        index += 1;
    }
    None
}

/// Some(true) when any Steam user has Steam Input on for ACE COMBAT 8 ("UseSteamControllerConfig" other than "0"),
/// Some(false) when it is set off, None when Steam has no setting for the game.
pub fn steam_input(steam: &Path) -> Option<bool> {
    let users = std::fs::read_dir(steam.join("userdata")).ok()?;
    let values: Vec<bool> = users
        .flatten()
        .filter_map(|user| std::fs::read_to_string(user.path().join("config").join("localconfig.vdf")).ok())
        .filter_map(|text| vdf_value(&text, AC8, "UseSteamControllerConfig"))
        .map(|value| value != "0")
        .collect();
    (!values.is_empty()).then(|| values.contains(&true))
}

/// The warnings to show when emulation starts.
pub fn warnings(runner: &Runner, system: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    if steam_running(runner, system) {
        warnings.push(STEAM_RUNNING.to_string());
    }
    if steam_path(runner, system).and_then(|steam| steam_input(&steam)) == Some(true) {
        warnings.push(STEAM_INPUT.to_string());
    }
    warnings
}
