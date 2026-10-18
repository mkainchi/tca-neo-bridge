use neo_daemon::config::{self, Binding, Control, Direction};
use neo_daemon::models;
use neo_daemon::shared::{Shared, Status};
use std::path::PathBuf;

type Change = Box<dyn Fn(&mut config::Config)>;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-config-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Defaults plus the TCA profile stored explicitly, so profile rules can be exercised.
fn with_tca() -> config::Config {
    let mut cfg = config::defaults();
    cfg.setups.insert(config::TCA_SETUP.into(), config::tca_profile());
    cfg
}

fn tca(cfg: &mut config::Config) -> &mut config::Profile {
    cfg.setups.get_mut(config::TCA_SETUP).unwrap()
}

#[test]
fn sources_name_a_device_and_a_control() {
    assert_eq!(config::source("tca-stick/b1"), Some(("tca-stick", Control::Button(0))));
    assert_eq!(config::source("t16000m#2/b128"), Some(("t16000m#2", Control::Button(127))));
    assert_eq!(config::source("044f-b10a/hat-up"), Some(("044f-b10a", Control::Hat(0))));
    assert_eq!(config::source("twcs/hat-left"), Some(("twcs", Control::Hat(3))));
    for bad in ["", "b1", "/b1", "x/hat-up-right", "x/b0", "x/b01", "x/b129", "x/b", "x/c1", "x/b1a", "x/b+1", "X/b1", "x#1/b1", "x/b99999999999999999999999"] {
        assert_eq!(config::source(bad), None, "{bad}");
    }
}

#[test]
fn device_ids_and_selection() {
    for good in ["tca-stick", "044f-b10a", "t16000m#2", "x#99", &"a".repeat(32)] {
        assert!(models::valid_id(good), "{good}");
    }
    for bad in ["", "A", "x y", "x#", "x#1", "x#02", "x#100", "x#a", &"a".repeat(33)] {
        assert!(!models::valid_id(bad), "{bad}");
    }
    assert_eq!(models::base_id(0x044f, 0x0406), "tca-stick");
    assert_eq!(models::base_id(0x044f, 0xb10a), "t16000m");
    assert_eq!(models::base_id(0x1234, 0xb10a), "1234-b10a");
    assert_eq!(models::model(0x044f, 0x2800), None);
    assert_eq!(models::strip("t16000m#2"), "t16000m");
    // Auto: every Thrustmaster joystick but the virtual NEO. An explicit list matches ids, any instance.
    assert!(models::wanted(&[], 0x044f, 0xb10a));
    assert!(!models::wanted(&[], 0x044f, models::NEO_PID));
    assert!(!models::wanted(&[], 0x1234, 0x0001));
    let list = ["1234-0001#2".to_string()];
    assert!(models::wanted(&list, 0x1234, 0x0001));
    assert!(!models::wanted(&list, 0x044f, 0xb10a));
    assert!(!models::wanted(&["044f-2800".into()], 0x044f, models::NEO_PID));
    let mut devices = vec![models::tca_stick(), models::tca_stick(), models::tca_quadrant(), models::tca_stick()];
    models::number(&mut devices);
    let ids: Vec<_> = devices.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["tca-stick", "tca-stick#2", "tca-quadrant", "tca-stick#3"]);
    assert_eq!(models::setup_key(&devices[..3]), "tca-quadrant+tca-stick+tca-stick#2");
    assert_eq!(models::tca_stick().layout.axis("rz"), Some(2));
}

#[test]
fn directions_map_to_hat_vectors() {
    assert_eq!(
        [Direction::Up, Direction::Down, Direction::Left, Direction::Right].map(Direction::vector),
        [(0, -1), (0, 1), (-1, 0), (1, 0)]
    );
}

#[test]
fn validation_rules() {
    assert!(config::validate(&config::defaults()).is_ok());
    assert!(config::validate(&with_tca()).is_ok());
    let cases: Vec<(Change, &str)> = vec![
        (Box::new(|c| c.version = 1), "version"),
        (Box::new(|c| c.deadzone = 1.0), "Deadzone"),
        (Box::new(|c| c.deadzone = -0.1), "Deadzone"),
        (Box::new(|c| c.camera_strength = 1.5), "Camera"),
        (Box::new(|c| c.devices = vec!["Bad".into()]), "device list"),
        (Box::new(|c| c.devices = vec!["x".into(); 17]), "device list"),
        (Box::new(|c| {
            for n in 0..33 {
                c.setups.insert(format!("s{n}"), config::tca_profile());
            }
        }), "32 controller sets"),
        (Box::new(|c| { c.setups.insert(String::new(), config::tca_profile()); }), "controller set"),
        (Box::new(|c| { c.setups.insert("a+B".into(), config::tca_profile()); }), "controller set"),
        (Box::new(|c| tca(c).split_high_g.back = 0.95), "Split High-G thresholds"),
        (Box::new(|c| tca(c).split_high_g.back = -0.1), "Split High-G thresholds"),
        (Box::new(|c| tca(c).split_high_g.forward = 1.2), "Split High-G thresholds"),
        (Box::new(|c| tca(c).split_high_g.levers.truncate(1)), "exactly two levers"),
        (Box::new(|c| tca(c).split_high_g.levers.clear()), "exactly two levers"),
        (Box::new(|c| tca(c).axes.throttle = vec![Binding::new("x", "z", false); 9]), "eight throttle"),
        (Box::new(|c| tca(c).axes.roll = Some(Binding::new("X", "x", false))), "Unknown axis X/x"),
        (Box::new(|c| tca(c).axes.throttle.push(Binding::new("x", "", false))), "Unknown axis x/"),
        (Box::new(|c| tca(c).split_high_g.levers[1].axis = "a".repeat(17)), "Unknown axis"),
        (Box::new(|c| tca(c).axes.yaw = Some(Binding::new("x", "Z", false))), "Unknown axis"),
        (Box::new(|c| tca(c).camera = Some("#".into())), "camera hat"),
        (Box::new(|c| { tca(c).buttons.insert("z9".into(), vec![1]); }), "Unknown control z9"),
        (Box::new(|c| { tca(c).buttons.insert("tca-stick/b3".into(), vec![16]); }), "tca-stick/b3: NEO buttons"),
        (Box::new(|c| { tca(c).buttons.insert("tca-stick/b3".into(), vec![0]); }), "tca-stick/b3: NEO buttons"),
        (Box::new(|c| { tca(c).pov.insert("q40".into(), Direction::Up); }), "Unknown control q40"),
    ];
    for (change, message) in cases {
        let mut cfg = with_tca();
        change(&mut cfg);
        assert!(config::validate(&cfg).unwrap_err().contains(message), "{message}");
    }
    // Split rule off with no levers, a camera on any valid id and axis names with digits are fine.
    let mut cfg = with_tca();
    tca(&mut cfg).axes.throttle.push(Binding::new("x", "slider2", false));
    tca(&mut cfg).split_high_g = config::SplitHighG::default();
    tca(&mut cfg).camera = None;
    assert!(config::validate(&cfg).is_ok());
}

#[test]
fn parse_accepts_v2_and_migrates_v1() {
    let text = config::to_json(&with_tca());
    assert_eq!(config::parse(&text).unwrap(), with_tca());
    assert!(config::parse("{").unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&text.replacen('{', "{\"extra\": 1,", 1)).unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&text.replace("\"version\": 2", "\"version\": 3")).unwrap_err().contains("version"));
    // Optional v2 fields default.
    let minimal = r#"{"version": 2, "deadzone": 0.1, "camera_strength": 0.5, "setups": {"x": {"buttons": {}, "pov": {}}}}"#;
    let parsed = config::parse(minimal).unwrap();
    assert!(parsed.devices.is_empty());
    assert_eq!(parsed.setups["x"].hat, config::HatMode::Camera);
    assert_eq!(parsed.setups["x"].split_high_g, config::SplitHighG::default());
    // v1: TCA keys move to the TCA devices; profiles saved before split/hat existed get split on at 75% / 25%, camera.
    let v1 = r#"{"version": 1, "deadzone": 0.04, "camera_strength": 1.0, "throttle_lever": "right",
        "invert": {"roll": true, "pitch": false, "yaw": false, "throttle": true},
        "buttons": {"s1": [1], "q31": [2], "hat-left": [3]}, "pov": {"q9": "up"}}"#;
    let migrated = config::parse(v1).unwrap();
    let profile = &migrated.setups[config::TCA_SETUP];
    assert_eq!(profile.buttons.keys().collect::<Vec<_>>(), ["tca-quadrant/b31", "tca-stick/b1", "tca-stick/hat-left"]);
    assert_eq!(profile.pov["tca-quadrant/b9"], Direction::Up);
    assert_eq!(profile.axes.roll, Some(Binding::new("tca-stick", "x", true)));
    assert_eq!(profile.axes.throttle, [Binding::new("tca-stick", "slider", true), Binding::new("tca-quadrant", "right", true)]);
    assert_eq!((profile.split_high_g.enabled, profile.split_high_g.forward, profile.split_high_g.back), (true, 0.75, 0.25));
    assert_eq!((profile.hat, profile.camera.as_deref()), (config::HatMode::Camera, Some("tca-stick")));
    let mapped = v1.replace("\"buttons\"", "\"hat\": \"mapped\", \"split_high_g\": {\"enabled\": false, \"forward\": 0.9, \"back\": 0.1}, \"buttons\"");
    let migrated = config::parse(&mapped).unwrap();
    let profile = &migrated.setups[config::TCA_SETUP];
    assert_eq!((profile.hat, profile.split_high_g.enabled, profile.split_high_g.forward), (config::HatMode::Mapped, false, 0.9));
    // Unknown v1 keys, bad v1 JSON and invalid migrated values are refused.
    assert!(config::parse(&v1.replace("\"q31\"", "\"q32\"")).unwrap_err().contains("Unknown TCA control q32"));
    assert!(config::parse(&v1.replace("\"q9\"", "\"z9\"")).unwrap_err().contains("Unknown TCA control z9"));
    assert!(config::parse(&v1.replace("\"right\"", "\"middle\"")).unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&v1.replace("0.04", "2.0")).unwrap_err().contains("Deadzone"));
    let mut direct: config::ConfigV1 = serde_json::from_str(v1).unwrap();
    direct.version = 7;
    assert!(config::migrate(direct).unwrap_err().contains("version"));
    assert_eq!(config::migrate_key("hat-up-right"), None);
    for bad in ["s", "s0", "s01", "s18", "q32", "s1a", "x1", "", "s99999999999999999999999"] {
        assert_eq!(config::migrate_key(bad), None, "{bad}");
    }
}

#[test]
fn profiles_are_kept_per_controller_set() {
    let pair = [models::tca_stick(), models::tca_quadrant()];
    let mut cfg = config::defaults();
    assert_eq!(cfg.profile(&pair), config::tca_profile());
    let mut changed = config::tca_profile();
    changed.hat = config::HatMode::Mapped;
    cfg.setups.insert(models::setup_key(&pair), changed.clone());
    assert_eq!(cfg.profile(&pair), changed);
    // The sidestick alone is another set, with the convention default.
    let alone = cfg.profile(&pair[..1]);
    assert_eq!(alone.axes.throttle, [Binding::new("tca-stick", "slider", false)]);
    assert!(!alone.split_high_g.enabled);
}

#[test]
fn load_save_round_trip_and_failures() {
    let dir = temp("roundtrip");
    let path = dir.join("config.json");
    assert_eq!(config::load(&path).unwrap(), config::defaults());
    let mut cfg = with_tca();
    cfg.deadzone = 0.1;
    config::save(&path, &cfg).unwrap();
    assert_eq!(config::load(&path).unwrap(), cfg);
    assert!(!dir.join("config.tmp").exists());
    cfg.deadzone = 3.0;
    assert!(config::save(&path, &cfg).unwrap_err().contains("Deadzone"));
    assert!(config::save(&dir.join("missing").join("config.json"), &config::defaults()).unwrap_err().contains("could not be saved"));
    std::fs::write(&path, "nope").unwrap();
    assert!(config::load(&path).is_err());
    assert_eq!(config::load(&dir).unwrap_err(), "The saved profile could not be read.");
}

#[test]
fn status_changes_are_logged_and_log_failures_are_harmless() {
    let dir = temp("log");
    let shared = Shared::new(dir.join("config.json"), config::defaults()).with_log(dir.join("neo-daemon.log"));
    shared.set_status(Status::Error("The virtual NEO did not appear in time.".into()));
    shared.set_status(Status::Idle);
    let text = std::fs::read_to_string(dir.join("neo-daemon.log")).unwrap();
    assert!(text.contains(" status Error: The virtual NEO did not appear in time.\n") && text.ends_with(" status Idle\n"));
    neo_daemon::shared::append(&dir, "a directory cannot be appended to");
}

#[test]
fn shared_state_publishes_generations_status_and_devices() {
    let dir = temp("shared");
    let shared = Shared::new(dir.join("config.json"), config::defaults());
    assert_eq!(shared.generation(), 0);
    assert_eq!(shared.present(), Ok(Vec::new())); // no scanner, no session
    let mut cfg = config::defaults();
    cfg.camera_strength = 0.25;
    shared.save(cfg.clone()).unwrap();
    assert_eq!(shared.config(), (1, cfg));
    let mut bad = config::defaults();
    bad.version = 0;
    assert!(shared.save(bad).is_err());
    assert_eq!(shared.generation(), 1);
    for (status, label, running) in [
        (Status::Idle, "Idle", false),
        (Status::Starting, "Starting", true),
        (Status::Emulating, "Emulating", true),
        (Status::Stopping, "Stopping", true),
        (Status::Error("x".into()), "Error: x", false),
    ] {
        shared.set_status(status.clone());
        assert_eq!((shared.status(), status.label(), status.running()), (status, label.to_string(), running));
    }
    *shared.devices.lock().unwrap() = Some(vec![models::tca_stick()]);
    assert_eq!(shared.present().unwrap()[0].id, "tca-stick"); // the session's devices win
}
