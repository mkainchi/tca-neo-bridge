use neo_daemon::hidhide::{self, Hiding, Journal, Runner};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const STICK: &str = r"HID\VID_044F&PID_0405&MI_00\7&ABC";
const QUADRANT: &str = r"HID\VID_044F&PID_0407\7&DEF";

#[derive(Clone)]
struct Cli {
    hidden: BTreeSet<String>,
    apps: Vec<String>,
    cloak: String,
    inverse: String,
    gaming: String,
    fail: Option<String>,
    skip: usize,
    garbage: Option<String>,
    log: Vec<String>,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            hidden: BTreeSet::new(),
            apps: Vec::new(),
            cloak: "--cloak-off".into(),
            inverse: "--inv-off".into(),
            gaming: gaming(&[(STICK, true, true), (QUADRANT, true, true)]),
            fail: None,
            skip: 0,
            garbage: None,
            log: Vec::new(),
        }
    }
}

fn gaming(devices: &[(&str, bool, bool)]) -> String {
    let items: Vec<serde_json::Value> = devices.iter().map(|(p, present, gaming)| serde_json::json!({"deviceInstancePath": p, "present": present, "gamingDevice": gaming})).collect();
    serde_json::json!([{"devices": items}]).to_string()
}

fn runner(cli: &Arc<Mutex<Cli>>) -> Runner {
    let cli = cli.clone();
    Box::new(move |_path: &Path, args: &[String]| {
        let mut c = cli.lock().unwrap();
        let command = args.join(" ");
        c.log.push(command.clone());
        if c.fail.as_deref() == Some(args[0].as_str()) {
            if c.skip == 0 {
                return Ok((Some(1), String::new()));
            }
            c.skip -= 1;
        }
        if c.garbage.as_deref() == Some(args[0].as_str()) {
            return Ok((Some(0), "garbage".into()));
        }
        let out = match args[0].as_str() {
            "--dev-list" => c.hidden.iter().map(|d| format!("--dev-hide \"{d}\"")).collect::<Vec<_>>().join("\n"),
            "--app-list" => c.apps.iter().map(|a| format!("--app-reg \"{a}\"")).collect::<Vec<_>>().join("\n"),
            "--cloak-state" => c.cloak.clone(),
            "--inv-state" => c.inverse.clone(),
            "--dev-gaming" => c.gaming.clone(),
            "--app-reg" => {
                c.apps.push(args[1].clone());
                String::new()
            }
            "--app-unreg" => {
                c.apps.retain(|a| a != &args[1]);
                String::new()
            }
            "--dev-hide" => {
                c.hidden.insert(args[1].clone());
                String::new()
            }
            "--dev-unhide" => {
                c.hidden.remove(&args[1]);
                String::new()
            }
            "--cloak-on" | "--cloak-off" => {
                c.cloak = args[0].clone();
                String::new()
            }
            "--io-error" => return Err(std::io::ErrorKind::Other.into()),
            _ => return Ok((None, String::new())),
        };
        Ok((Some(0), out))
    })
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-hidhide-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn apps() -> Vec<String> {
    vec![r"C:\neo\neo-daemon.exe".into(), r"C:\neo\neo-daemon.exe".into()]
}

fn hiding(cli: &Arc<Mutex<Cli>>, dir: &Path) -> Result<Hiding, String> {
    Hiding::new(Some(PathBuf::from("HidHideCLI.exe")), runner(cli), dir, &apps())
}

#[test]
fn installation_finds_either_vendor_folder() {
    let dir = temp("install");
    assert_eq!(hidhide::installation(&dir), None);
    let path = dir.join("Nefarius Software Solutions e.U.").join("HidHide").join("x64");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("HidHideCLI.exe"), b"").unwrap();
    assert_eq!(hidhide::installation(&dir), Some(path.join("HidHideCLI.exe")));
}

#[test]
fn real_cli_runner_reports_output_exit_and_timeout() {
    let cmd = PathBuf::from(std::env::var("ComSpec").unwrap());
    let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    assert_eq!(hidhide::run_cli(&cmd, &args("/c echo --inv-off"), Duration::from_secs(10)).unwrap(), (Some(0), "--inv-off".into()));
    assert_eq!(hidhide::run_cli(&cmd, &args("/c exit 3"), Duration::from_secs(10)).unwrap().0, Some(3));
    let slow = hidhide::run_cli(&cmd, &args("/c ping -n 5 127.0.0.1"), Duration::from_millis(50)).unwrap_err();
    assert_eq!(slow.kind(), std::io::ErrorKind::TimedOut);
    assert!(hidhide::run_cli(Path::new("missing-hidhide-cli.exe"), &[], Duration::from_secs(1)).is_err());
    let real = hidhide::real_runner();
    assert_eq!(real(&cmd, &args("/c echo ok")).unwrap().1, "ok");
}

#[test]
fn missing_cli_is_reported() {
    let error = Hiding::new(None, runner(&Arc::new(Mutex::new(Cli::default()))), &temp("missing"), &[]).err().unwrap();
    assert!(error.contains("Install HidHide"));
}

#[test]
fn session_hides_registers_verifies_and_restores_exactly() {
    let dir = temp("session");
    let cli = Arc::new(Mutex::new(Cli { apps: vec![r"C:\other.exe".into()], ..Default::default() }));
    let mut hiding = hiding(&cli, &dir).unwrap();
    hiding.start().unwrap();
    {
        let c = cli.lock().unwrap();
        assert_eq!(c.cloak, "--cloak-on");
        assert!(c.hidden.contains(STICK) && c.hidden.contains(QUADRANT));
        assert_eq!(c.apps.iter().filter(|a| a.ends_with("neo-daemon.exe")).count(), 1);
    }
    let journal: Journal = serde_json::from_str(&std::fs::read_to_string(dir.join("hidhide-session.json")).unwrap()).unwrap();
    assert_eq!(journal.active, "--cloak-off");
    hiding.verify().unwrap();
    hiding.close().unwrap();
    hiding.close().unwrap(); // idempotent
    let c = cli.lock().unwrap();
    assert_eq!((c.cloak.as_str(), c.hidden.len(), c.apps.clone()), ("--cloak-off", 0, vec![r"C:\other.exe".to_string()]));
    assert!(!dir.join("hidhide-session.json").exists());
}

#[test]
fn already_hidden_devices_and_registered_apps_are_left_alone() {
    let dir = temp("existing");
    let cli = Arc::new(Mutex::new(Cli { hidden: [STICK.to_string()].into(), apps: vec![r"c:\NEO\neo-daemon.exe".into()], cloak: "--cloak-on".into(), ..Default::default() }));
    let mut hiding = hiding(&cli, &dir).unwrap();
    hiding.start().unwrap();
    hiding.close().unwrap();
    let c = cli.lock().unwrap();
    assert_eq!((c.cloak.as_str(), c.hidden.len(), c.apps.len()), ("--cloak-on", 1, 1));
}

#[test]
fn start_refusals() {
    let cases: Vec<(Cli, &str)> = vec![
        (Cli { inverse: "--inv-on".into(), ..Default::default() }, "inverse"),
        (Cli { cloak: "weird".into(), ..Default::default() }, "inverse"),
        (Cli { apps: vec![r"C:\Steam\steam.exe".into()], ..Default::default() }, "Remove Steam"),
        (Cli { gaming: gaming(&[(STICK, true, true)]), ..Default::default() }, "both supported"),
        (Cli { gaming: gaming(&[(QUADRANT, true, true), (r"HID\VID_044F&PID_04060", true, true)]), ..Default::default() }, "both supported"),
        (Cli { gaming: gaming(&[(STICK, false, true), (QUADRANT, true, true)]), ..Default::default() }, "both supported"),
        (Cli { gaming: gaming(&[(STICK, true, false), (QUADRANT, true, true)]), ..Default::default() }, "both supported"),
        (Cli { hidden: ["OTHER".to_string()].into(), ..Default::default() }, "other hidden devices"),
        (Cli { gaming: "{}".into(), ..Default::default() }, "isolation failed"),
        (Cli { gaming: "[".into(), ..Default::default() }, "isolation failed"),
        (Cli { gaming: r#"[{"devices":[{"present":true}]}]"#.into(), ..Default::default() }, "isolation failed"),
        (Cli { garbage: Some("--dev-list".into()), ..Default::default() }, "isolation failed"),
        (Cli { hidden: [String::new()].into(), ..Default::default() }, "isolation failed"),
        (Cli { hidden: ["a\"b".to_string()].into(), ..Default::default() }, "isolation failed"),
        (Cli { garbage: Some("--app-list".into()), ..Default::default() }, "isolation failed"),
    ];
    for (state, message) in cases {
        let dir = temp("refusal");
        let mut hiding = hiding(&Arc::new(Mutex::new(state)), &dir).unwrap();
        assert!(hiding.start().unwrap_err().contains(message), "{message}");
    }
}

#[test]
fn group_without_device_list_is_ignored() {
    let dir = temp("nogroup");
    let mut state = Cli::default();
    let mut groups: Vec<serde_json::Value> = serde_json::from_str(&state.gaming).unwrap();
    groups.push(serde_json::json!({"name": "empty"}));
    state.gaming = serde_json::to_string(&groups).unwrap();
    hiding(&Arc::new(Mutex::new(state)), &dir).unwrap().start().unwrap();
}

#[test]
fn every_cli_failure_surfaces_the_isolation_error() {
    for failing in ["--dev-list", "--app-list", "--cloak-state", "--inv-state", "--dev-gaming", "--app-reg", "--dev-hide", "--cloak-on"] {
        let dir = temp("fail");
        let cli = Arc::new(Mutex::new(Cli { fail: Some(failing.into()), ..Default::default() }));
        assert_eq!(hiding(&cli, &dir).unwrap().start().unwrap_err(), hidhide::ERROR, "{failing}");
    }
    let dir = temp("secondgaming");
    let cli = Arc::new(Mutex::new(Cli { fail: Some("--dev-gaming".into()), skip: 1, ..Default::default() }));
    assert_eq!(hiding(&cli, &dir).unwrap().start().unwrap_err(), hidhide::ERROR);
    for failing in ["--app-list", "--cloak-state", "--inv-state"] {
        let dir = temp("verifycall");
        let cli = Arc::new(Mutex::new(Cli::default()));
        let mut h = hiding(&cli, &dir).unwrap();
        h.start().unwrap();
        cli.lock().unwrap().fail = Some(failing.into());
        assert_eq!(h.verify().unwrap_err(), hidhide::ERROR, "{failing}");
    }
    // Second-call failures inside verify (the state flips after start).
    for (field, value) in [("cloak", "--cloak-off"), ("inverse", "--inv-on")] {
        let dir = temp("verify");
        let cli = Arc::new(Mutex::new(Cli::default()));
        let mut h = hiding(&cli, &dir).unwrap();
        h.start().unwrap();
        let mut c = cli.lock().unwrap();
        if field == "cloak" { c.cloak = value.into() } else { c.inverse = value.into() }
        drop(c);
        assert_eq!(h.verify().unwrap_err(), hidhide::ERROR);
    }
    for change in 0..4 {
        let dir = temp("verify2");
        let cli = Arc::new(Mutex::new(Cli::default()));
        let mut h = hiding(&cli, &dir).unwrap();
        h.start().unwrap();
        let mut c = cli.lock().unwrap();
        match change {
            0 => { c.hidden.clear(); }
            1 => { c.apps.clear(); }
            2 => { c.apps.push(r"D:\steamwebhelper.exe".into()); }
            _ => { c.fail = Some("--dev-list".into()); }
        }
        drop(c);
        assert_eq!(h.verify().unwrap_err(), hidhide::ERROR);
    }
}

#[test]
fn close_failures_and_unwritable_journal() {
    for failing in ["--dev-unhide", "--cloak-off", "--app-unreg"] {
        let dir = temp("closefail");
        let cli = Arc::new(Mutex::new(Cli::default()));
        let mut h = hiding(&cli, &dir).unwrap();
        h.start().unwrap();
        cli.lock().unwrap().fail = Some(failing.into());
        assert_eq!(h.close().unwrap_err(), hidhide::ERROR);
    }
    let dir = temp("journalgone");
    let cli = Arc::new(Mutex::new(Cli::default()));
    let mut h = hiding(&cli, &dir).unwrap();
    h.start().unwrap();
    std::fs::remove_file(dir.join("hidhide-session.json")).unwrap();
    assert_eq!(h.close().unwrap_err(), hidhide::ERROR);
    let dir = temp("nodir").join("absent");
    let mut h = Hiding::new(Some("x".into()), runner(&cli), &dir, &[]).unwrap();
    cli.lock().unwrap().hidden.clear();
    cli.lock().unwrap().cloak = "--cloak-off".into();
    assert_eq!(h.start().unwrap_err(), hidhide::ERROR);
}

#[test]
fn unexpected_exit_and_io_errors_are_failures() {
    let cli = Arc::new(Mutex::new(Cli::default()));
    let run = runner(&cli);
    assert_eq!(run(Path::new("x"), &["--unknown".into()]).unwrap(), (None, String::new()));
    assert!(run(Path::new("x"), &["--io-error".into()]).is_err());
}

#[test]
fn interrupted_session_journal_is_recovered_or_rejected() {
    let dir = temp("recover");
    let cli = Arc::new(Mutex::new(Cli { hidden: [STICK.to_string()].into(), cloak: "--cloak-on".into(), apps: vec!["A".into()], ..Default::default() }));
    let journal = Journal { devices: vec![STICK.into()], apps: vec!["A".into()], active: "--cloak-off".into() };
    std::fs::write(dir.join("hidhide-session.json"), serde_json::to_string(&journal).unwrap()).unwrap();
    hiding(&cli, &dir).unwrap();
    let c = cli.lock().unwrap().clone();
    assert_eq!((c.hidden.len(), c.cloak.as_str(), c.apps.len()), (0, "--cloak-off", 0));
    for bad in [r#"{"devices":[],"apps":[],"active":"--other"}"#, r#"{"devices":[""],"apps":[],"active":"--cloak-on"}"#, "nope"] {
        std::fs::write(dir.join("hidhide-session.json"), bad).unwrap();
        assert!(hiding(&cli, &dir).err().unwrap().contains("recovery data is invalid"));
    }
    std::fs::write(dir.join("hidhide-session.json"), serde_json::to_string(&journal).unwrap()).unwrap();
    cli.lock().unwrap().fail = Some("--dev-unhide".into());
    assert_eq!(hiding(&cli, &dir).err().unwrap(), hidhide::ERROR);
}

#[test]
fn a_reconnected_tca_on_a_new_port_is_hidden_and_journaled() {
    let dir = temp("rehide");
    let cli = Arc::new(Mutex::new(Cli::default()));
    let mut h = hiding(&cli, &dir).unwrap();
    assert_eq!(h.rehide().unwrap_err(), hidhide::ERROR); // not started: nothing to add to
    h.start().unwrap();
    // The quadrant comes back as a new device instance (another USB port).
    const MOVED: &str = r"HID\VID_044F&PID_0407\8&NEW";
    cli.lock().unwrap().gaming = gaming(&[(STICK, true, true), (MOVED, true, true)]);
    h.rehide().unwrap();
    assert!(cli.lock().unwrap().hidden.contains(MOVED));
    let journal: Journal = serde_json::from_str(&std::fs::read_to_string(dir.join("hidhide-session.json")).unwrap()).unwrap();
    assert!(journal.devices.contains(&MOVED.to_string()));
    h.rehide().unwrap(); // nothing new the second time
    h.close().unwrap();
    assert!(cli.lock().unwrap().hidden.is_empty());
    // Failures surface the isolation error.
    for failing in ["--dev-gaming", "--dev-list", "--dev-hide"] {
        let dir = temp("rehide-fail");
        let cli = Arc::new(Mutex::new(Cli::default()));
        let mut h = hiding(&cli, &dir).unwrap();
        h.start().unwrap();
        cli.lock().unwrap().gaming = gaming(&[(STICK, true, true), (MOVED, true, true)]);
        cli.lock().unwrap().fail = Some(failing.into());
        assert_eq!(h.rehide().unwrap_err(), hidhide::ERROR, "{failing}");
        cli.lock().unwrap().fail = None;
        h.close().unwrap();
    }
    let dir = temp("rehide-journal");
    let cli = Arc::new(Mutex::new(Cli::default()));
    let mut h = hiding(&cli, &dir).unwrap();
    h.start().unwrap();
    std::fs::create_dir_all(dir.join("hidhide-session.tmp")).unwrap(); // the journal cannot be written
    assert_eq!(h.rehide().unwrap_err(), hidhide::ERROR);
}
