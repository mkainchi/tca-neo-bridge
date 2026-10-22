//! Session-owned HidHide changes (port of app/hiding.py); private identifiers never reach messages.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub use crate::process::{run_cli, Runner};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const ERROR: &str = "HidHide isolation failed. Close its configuration client, recheck HidHide and reconnect the controllers. Close Steam and the game before starting emulation.";

pub fn installation(program_files: &Path) -> Option<PathBuf> {
    ["Nefarius Software Solutions", "Nefarius Software Solutions e.U."]
        .iter()
        .flat_map(|vendor| ["HidHideCLI.exe", "x64/HidHideCLI.exe"].map(|name| program_files.join(vendor).join("HidHide").join(name)))
        .find(|path| path.is_file())
}

pub fn real_runner() -> Runner {
    crate::process::runner(Duration::from_secs(5))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Journal {
    pub devices: Vec<String>,
    pub apps: Vec<String>,
    pub active: String,
}

pub struct Hiding {
    cli: PathBuf,
    runner: Runner,
    journal: PathBuf,
    applications: Vec<String>,
    state: Option<Journal>,
    expected: BTreeSet<String>,
}

/// "VID_xxxx&PID_yyyy" of an (uppercase) instance path, with both fields complete.
pub fn ids(path: &str) -> Option<(u16, u16)> {
    let at = path.find("VID_")?;
    let (vid, rest) = path[at + 4..].split_at_checked(4)?;
    let pid = rest.strip_prefix("&PID_")?.get(..4)?;
    if !matches!(rest.as_bytes().get(9), None | Some(b'&' | b'\\')) {
        return None;
    }
    let hex = |text: &str| u16::from_str_radix(text, 16).ok().filter(|_| text.bytes().all(|b| b.is_ascii_hexdigit()));
    Some((hex(vid)?, hex(pid)?))
}

fn steam(path: &str) -> bool {
    let name = Path::new(path).file_name().map(|n| n.to_string_lossy().to_lowercase());
    matches!(name.as_deref(), Some("steam.exe" | "steamwebhelper.exe"))
}

impl Hiding {
    /// Recovers (restores) a journal left by an interrupted session before anything else.
    pub fn new(cli: Option<PathBuf>, runner: Runner, local: &Path, applications: &[String]) -> Result<Self, String> {
        let cli = cli.ok_or("Install HidHide before emulating.")?;
        let mut seen = BTreeSet::new();
        let applications: Vec<String> = applications.iter().filter(|a| seen.insert(a.to_string())).cloned().collect();
        let mut hiding = Self { cli, runner, journal: local.join("hidhide-session.json"), applications, state: None, expected: BTreeSet::new() };
        if let Ok(text) = std::fs::read_to_string(&hiding.journal) {
            let state: Journal = serde_json::from_str(&text)
                .ok()
                .filter(|j: &Journal| ["--cloak-on", "--cloak-off"].contains(&j.active.as_str()) && j.devices.iter().chain(&j.apps).all(|v| !v.is_empty()))
                .ok_or("HidHide recovery data is invalid. Restore controller visibility in its configuration client before removing the private recovery journal.")?;
            hiding.state = Some(state);
            hiding.close()?;
        }
        Ok(hiding)
    }

    fn call(&self, args: &[&str]) -> Result<String, String> {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        match (self.runner)(&self.cli, &args) {
            Ok((Some(0), stdout)) => Ok(stdout),
            _ => Err(ERROR.into()),
        }
    }

    fn entries(&self, kind: &str) -> Result<BTreeSet<String>, String> {
        let text = self.call(&[&format!("--{kind}-list")])?;
        let prefix = if kind == "dev" { "--dev-hide \"" } else { "--app-reg \"" };
        text.lines()
            .map(|line| {
                let value = line.strip_prefix(prefix).and_then(|rest| rest.strip_suffix('"')).filter(|v| !v.is_empty() && !v.contains('"'));
                value.map(|v| if kind == "dev" { v.to_uppercase() } else { v.to_string() }).ok_or_else(|| ERROR.to_string())
            })
            .collect()
    }

    fn save(&self) -> Result<(), String> {
        let temporary = self.journal.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_string(&self.state).unwrap())
            .and_then(|()| std::fs::rename(&temporary, &self.journal))
            .map_err(|_| ERROR.to_string())
    }

    fn devices(&self, wanted: &dyn Fn(u16, u16) -> bool) -> Result<BTreeSet<String>, String> {
        let data: serde_json::Value = serde_json::from_str(&self.call(&["--dev-gaming"])?).map_err(|_| ERROR.to_string())?;
        let groups = data.as_array().ok_or(ERROR)?;
        let mut found = BTreeSet::new();
        for device in groups.iter().flat_map(|g| g["devices"].as_array().into_iter().flatten()) {
            let path = device["deviceInstancePath"].as_str().ok_or(ERROR)?.to_uppercase();
            let matches = ids(&path).is_some_and(|(vid, pid)| wanted(vid, pid));
            if device["present"] == true && device["gamingDevice"] == true && matches {
                found.insert(path);
            }
        }
        Ok(found)
    }

    /// Hide every present gaming device `wanted` accepts (by vendor and product id).
    pub fn start(&mut self, wanted: &dyn Fn(u16, u16) -> bool) -> Result<(), String> {
        let hidden = self.entries("dev")?;
        let apps = self.entries("app")?;
        let active = self.call(&["--cloak-state"])?;
        if self.call(&["--inv-state"])? != "--inv-off" || !["--cloak-on", "--cloak-off"].contains(&active.as_str()) {
            return Err("HidHide requires normal application hiding. Turn inverse application cloak off.".into());
        }
        if apps.iter().any(|p| steam(p)) {
            return Err("Remove Steam from the HidHide Applications list before emulating.".into());
        }
        let devices = self.devices(wanted)?;
        if devices.is_empty() {
            return Err("HidHide must detect the selected controllers. Reconnect them and retry.".into());
        }
        if active == "--cloak-off" && !hidden.is_subset(&devices) {
            return Err("HidHide has other hidden devices. Enable hiding yourself or clear unused entries before emulating.".into());
        }
        let registered: BTreeSet<String> = apps.iter().map(|a| a.to_lowercase()).collect();
        self.state = Some(Journal {
            devices: devices.difference(&hidden).cloned().collect(),
            apps: self.applications.iter().filter(|p| !registered.contains(&p.to_lowercase())).cloned().collect(),
            active,
        });
        self.save()?;
        let state = self.state.clone().unwrap();
        for app in &state.apps {
            self.call(&["--app-reg", app])?;
        }
        for device in &state.devices {
            self.call(&["--dev-hide", device])?;
        }
        self.call(&["--cloak-on"])?;
        self.expected = devices;
        self.verify()
    }

    pub fn verify(&self) -> Result<(), String> {
        let apps: BTreeSet<String> = self.entries("app")?.iter().map(|a| a.to_lowercase()).collect();
        let ok = self.call(&["--cloak-state"])? == "--cloak-on"
            && self.call(&["--inv-state"])? == "--inv-off"
            && self.expected.is_subset(&self.entries("dev")?)
            && self.applications.iter().all(|p| apps.contains(&p.to_lowercase()))
            && !apps.iter().any(|p| steam(p));
        if ok {
            Ok(())
        } else {
            Err(ERROR.into())
        }
    }

    /// After a controller reconnects, possibly on another USB port as a new device instance: journal and hide every
    /// wanted device not hidden yet, so `close` unhides it too, then check the result.
    pub fn rehide(&mut self, wanted: &dyn Fn(u16, u16) -> bool) -> Result<(), String> {
        let found = self.devices(wanted)?;
        let fresh: Vec<String> = found.difference(&self.entries("dev")?).cloned().collect();
        self.state.as_mut().ok_or(ERROR)?.devices.extend(fresh.iter().cloned());
        self.save()?;
        for device in &fresh {
            self.call(&["--dev-hide", device])?;
        }
        self.expected.extend(found);
        self.verify()
    }

    pub fn close(&mut self) -> Result<(), String> {
        let Some(state) = self.state.clone() else { return Ok(()) };
        for device in &state.devices {
            self.call(&["--dev-unhide", device])?;
        }
        self.call(&[&state.active])?;
        for app in &state.apps {
            self.call(&["--app-unreg", app])?;
        }
        std::fs::remove_file(&self.journal).map_err(|_| ERROR.to_string())?;
        self.state = None;
        Ok(())
    }
}
