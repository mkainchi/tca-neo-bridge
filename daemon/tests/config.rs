use neo_daemon::config::{self, Device, Direction};
use neo_daemon::shared::{Shared, Status};
use std::path::PathBuf;

type Change = Box<dyn Fn(&mut config::Config)>;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-config-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn sources_cover_both_devices_and_reject_everything_else() {
    assert_eq!(config::source("s1"), Some((Device::Stick, 0)));
    assert_eq!(config::source("s17"), Some((Device::Stick, 16)));
    assert_eq!(config::source("q31"), Some((Device::Quadrant, 30)));
    assert_eq!(config::source("hat-up"), Some((Device::Hat, 0)));
    assert_eq!(config::source("hat-left"), Some((Device::Hat, 3)));
    assert_eq!(config::source("hat-up-right"), None);
    for bad in ["", "x1", "s", "s0", "s01", "s18", "q32", "s1a", "s+1", "s99999999999999999999999"] {
        assert_eq!(config::source(bad), None, "{bad}");
    }
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
    let cases: Vec<(Change, &str)> = vec![
        (Box::new(|c| c.version = 2), "version"),
        (Box::new(|c| c.deadzone = 1.0), "Deadzone"),
        (Box::new(|c| c.deadzone = -0.1), "Deadzone"),
        (Box::new(|c| c.camera_strength = 1.5), "Camera"),
        (Box::new(|c| c.split_high_g.back = 0.95), "Split High-G"),
        (Box::new(|c| c.split_high_g.back = -0.1), "Split High-G"),
        (Box::new(|c| c.split_high_g.forward = 1.2), "Split High-G"),
        (Box::new(|c| { c.buttons.insert("z9".into(), vec![1]); }), "Unknown TCA control z9"),
        (Box::new(|c| { c.buttons.insert("s3".into(), vec![16]); }), "s3: NEO buttons"),
        (Box::new(|c| { c.buttons.insert("s3".into(), vec![0]); }), "s3: NEO buttons"),
        (Box::new(|c| { c.pov.insert("q40".into(), Direction::Up); }), "Unknown TCA control q40"),
    ];
    for (change, message) in cases {
        let mut cfg = config::defaults();
        change(&mut cfg);
        assert!(config::validate(&cfg).unwrap_err().contains(message), "{message}");
    }
}

#[test]
fn parse_rejects_bad_json_unknown_fields_and_invalid_values() {
    let text = config::to_json(&config::defaults());
    assert_eq!(config::parse(&text).unwrap(), config::defaults());
    assert!(config::parse("{").unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&text.replacen('{', "{\"extra\": 1,", 1)).unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&text.replace("\"version\": 1", "\"version\": 3")).unwrap_err().contains("version"));
    // Profiles saved before the split setting existed get it enabled at 75% / 25%.
    let without_split: serde_json::Value = {
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value.as_object_mut().unwrap().remove("split_high_g");
        value
    };
    let legacy_split = config::parse(&without_split.to_string()).unwrap().split_high_g;
    assert_eq!((legacy_split.enabled, legacy_split.forward, legacy_split.back), (true, 0.75, 0.25));
    // Profiles saved before the hat setting existed default to the camera.
    let legacy = text.replace("\"hat\": \"camera\",", "");
    assert_ne!(legacy, text);
    assert_eq!(config::parse(&legacy).unwrap().hat, config::HatMode::Camera);
    assert_eq!(config::parse(&text.replace("\"hat\": \"camera\"", "\"hat\": \"mapped\"")).unwrap().hat, config::HatMode::Mapped);
}

#[test]
fn load_save_round_trip_and_failures() {
    let dir = temp("roundtrip");
    let path = dir.join("config.json");
    assert_eq!(config::load(&path).unwrap(), config::defaults());
    let mut cfg = config::defaults();
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
fn shared_state_publishes_generations_and_status() {
    let dir = temp("shared");
    let shared = Shared::new(dir.join("config.json"), config::defaults());
    assert_eq!(shared.generation(), 0);
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
}
