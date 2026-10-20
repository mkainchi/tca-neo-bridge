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

fn target(buttons: &[u8]) -> config::Target {
    config::Target { buttons: buttons.to_vec(), pov: None }
}

#[test]
fn gestures_layers_camera_and_yaw_round_trip_and_default_when_absent() {
    let mut cfg = config::defaults();
    cfg.long_press.insert("s2".into(), target(&[3]));
    cfg.double_tap.insert("s2".into(), config::Target { buttons: vec![], pov: Some(Direction::Left) });
    cfg.shift = Some("s16".into());
    cfg.shifted.buttons.insert("s1".into(), vec![9]);
    cfg.shifted.pov.insert("s3".into(), Direction::Down);
    cfg.behaviours.insert("s7".into(), config::Mode::Repeat);
    cfg.camera = config::Camera { ramp_ms: 300, full_after_hold: true, lever: false };
    cfg.yaw_source = config::YawSource::Off;
    assert_eq!(config::parse(&config::to_json(&cfg)).unwrap(), cfg);
    cfg.split_high_g.enabled = false;
    cfg.camera.lever = true;
    assert!(config::validate(&cfg).is_ok());
    cfg.camera.lever = false;
    cfg.yaw_source = config::YawSource::LeverDifferential;
    assert!(config::validate(&cfg).is_ok());
    // A version 2 profile saved before these settings existed: timing keeps its pulse and gains the defaults.
    let mut value: serde_json::Value = serde_json::from_str(&config::to_json(&config::defaults())).unwrap();
    let object = value.as_object_mut().unwrap();
    for key in ["long_press", "double_tap", "shift", "shifted", "camera", "yaw_source"] {
        object.remove(key);
    }
    object.insert("timing".into(), serde_json::json!({"pulse_ms": 60}));
    let early = config::parse(&value.to_string()).unwrap();
    assert_eq!(early.timing, config::Timing { pulse_ms: 60, long_ms: 500, double_ms: 250, repeat_ms: 120 });
    assert_eq!((early.shift, early.camera, early.yaw_source), (None, config::Camera::default(), config::YawSource::Twist));
}

#[test]
fn validation_rules() {
    assert!(config::validate(&config::defaults()).is_ok());
    let cases: Vec<(Change, &str)> = vec![
        (Box::new(|c| c.version = 1), "version"),
        (Box::new(|c| c.axes.roll.deadzone = 0.6), "Roll deadzone"),
        (Box::new(|c| c.axes.pitch.deadzone = -0.1), "Pitch deadzone"),
        (Box::new(|c| c.axes.yaw.saturation = 0.4), "Yaw full deflection"),
        (Box::new(|c| c.axes.yaw.saturation = 1.1), "Yaw full deflection"),
        (Box::new(|c| { c.axes.roll.deadzone = 0.5; c.axes.roll.saturation = 0.5; }), "Roll full deflection"),
        (Box::new(|c| c.axes.roll.anti_deadzone = 0.6), "Roll anti-deadzone"),
        (Box::new(|c| c.axes.roll.curve = -1.5), "Roll curve"),
        (Box::new(|c| c.axes.roll.smoothing = 0.95), "Roll smoothing"),
        (Box::new(|c| c.camera_strength = 1.5), "Camera"),
        (Box::new(|c| c.throttle.band = 0.25), "Throttle detents"),
        (Box::new(|c| c.throttle.band = -0.01), "Throttle detents"),
        (Box::new(|c| c.throttle.idle = -0.1), "Throttle detents"),
        (Box::new(|c| c.throttle.idle = 0.47), "Throttle detents"),
        (Box::new(|c| c.throttle.boost = 0.53), "Throttle detents"),
        (Box::new(|c| c.throttle.boost = 1.1), "Throttle detents"),
        (Box::new(|c| c.precision.source = Some("s40".into())), "Unknown TCA control s40"),
        (Box::new(|c| c.precision.scale = 0.05), "Precision scale"),
        (Box::new(|c| c.combos.pause = ["s1", "s2", "s3", "s4", "s5"].map(String::from).to_vec()), "at most 4"),
        (Box::new(|c| c.combos.pause = vec!["q99".into()]), "Unknown TCA control q99"),
        (Box::new(|c| { c.behaviours.insert("x".into(), config::Mode::Toggle); }), "Unknown TCA control x"),
        (Box::new(|c| c.timing.pulse_ms = 10), "Pulse length"),
        (Box::new(|c| c.timing.pulse_ms = 2000), "Pulse length"),
        (Box::new(|c| c.timing.long_ms = 100), "long press needs"),
        (Box::new(|c| c.timing.long_ms = 3000), "long press needs"),
        (Box::new(|c| c.timing.double_ms = 50), "long press needs"),
        (Box::new(|c| c.timing.double_ms = 1500), "long press needs"),
        (Box::new(|c| c.timing.repeat_ms = 20), "Repeat interval"),
        (Box::new(|c| c.timing.repeat_ms = 2000), "Repeat interval"),
        (Box::new(|c| c.timing.repeat_ms = 80), "Repeat interval"),
        (Box::new(|c| { c.long_press.insert("s99".into(), target(&[1])); }), "Unknown TCA control s99"),
        (Box::new(|c| { c.long_press.insert("s2".into(), target(&[16])); }), "s2: NEO buttons"),
        (Box::new(|c| { c.double_tap.insert("s2".into(), target(&[0])); }), "s2: NEO buttons"),
        (Box::new(|c| { c.shifted.buttons.insert("q1".into(), vec![20]); }), "q1: NEO buttons"),
        (Box::new(|c| { c.shifted.pov.insert("nope".into(), Direction::Up); }), "Unknown TCA control nope"),
        (Box::new(|c| c.shift = Some("s0".into())), "Unknown TCA control s0"),
        (Box::new(|c| c.camera.ramp_ms = 2500), "Camera ramp"),
        (Box::new(|c| c.camera.lever = true), "turn on only one"),
        (Box::new(|c| c.yaw_source = config::YawSource::LeverDifferential), "turn on only one"),
        (Box::new(|c| { c.split_high_g.enabled = false; c.camera.lever = true; c.yaw_source = config::YawSource::LeverDifferential; }), "turn on only one"),
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
    assert!(config::parse(&text.replace("\"version\": 2", "\"version\": 3")).unwrap_err().contains("version"));
    assert!(config::parse("[]").unwrap_err().starts_with("Invalid profile"));
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

/// A profile exactly as version 0.2.2 saved it.
const V1: &str = r#"{"version": 1, "deadzone": 0.07, "camera_strength": 0.5, "throttle_lever": "right", "hat": "mapped",
    "split_high_g": {"enabled": false, "forward": 0.8, "back": 0.2}, "invert": {"roll": false, "pitch": true, "yaw": false, "throttle": false},
    "buttons": {"q3": [6], "s1": [1]}, "pov": {"s11": "up"}}"#;

#[test]
fn version_1_profiles_migrate_to_per_axis_tuning_and_keep_holding() {
    let cfg = config::parse(V1).unwrap();
    assert_eq!(cfg.version, config::VERSION);
    let axis = config::AxisTuning { deadzone: 0.07, ..config::AxisTuning::default() };
    assert_eq!(cfg.axes, config::Axes { roll: axis, pitch: axis, yaw: axis });
    assert!(cfg.behaviours.is_empty()); // the latching switches keep holding, as before
    assert_eq!((cfg.camera_strength, cfg.throttle_lever, cfg.hat, cfg.invert.pitch), (0.5, config::Lever::Right, config::HatMode::Mapped, true));
    assert!(!cfg.split_high_g.enabled && cfg.safe_start);
    assert_eq!((cfg.throttle, cfg.precision.clone(), cfg.combos.clone(), cfg.timing), Default::default());
    assert_eq!((cfg.buttons["q3"].clone(), cfg.pov["s11"]), (vec![6], Direction::Up));
    // A v1 deadzone out of the v2 range is reported, not clamped; a v1 profile without one is invalid.
    assert!(config::parse(&V1.replace("0.07", "0.8")).unwrap_err().contains("Roll deadzone"));
    assert!(config::parse(&V1.replace("\"deadzone\": 0.07,", "")).unwrap_err().starts_with("Invalid profile"));
}

#[test]
fn defaults_pulse_the_latching_switches_and_arm_a_safe_start() {
    let cfg = config::defaults();
    let modes: Vec<_> = cfg.behaviours.iter().map(|(k, m)| (k.as_str(), *m)).collect();
    use config::Mode::{Pulse, PulseChange};
    assert_eq!(modes, [("q3", PulseChange), ("q4", PulseChange), ("q7", Pulse), ("q8", Pulse)]);
    assert!(cfg.safe_start && cfg.precision.source.is_none() && cfg.combos.pause.is_empty());
    assert_eq!((cfg.throttle.mode, cfg.timing.pulse_ms), (config::ThrottleMode::Linear, 80));
    // Settings added in version 2 may be left out of a v2 profile.
    let mut value: serde_json::Value = serde_json::from_str(&config::to_json(&cfg)).unwrap();
    for key in ["throttle", "precision", "safe_start", "combos", "behaviours", "timing"] {
        value.as_object_mut().unwrap().remove(key);
    }
    let sparse = config::parse(&value.to_string()).unwrap();
    assert!(sparse.safe_start && sparse.behaviours.is_empty());
    let mut valid = cfg.clone();
    valid.precision.source = Some("hat-down".into());
    valid.combos.pause = vec!["s5".into(), "q6".into()];
    assert_eq!(config::parse(&config::to_json(&valid)).unwrap(), valid);
}

#[test]
fn load_save_round_trip_and_failures() {
    let dir = temp("roundtrip");
    let path = dir.join("config.json");
    assert_eq!(config::load(&path).unwrap(), config::defaults());
    let mut cfg = config::defaults();
    cfg.axes.roll.deadzone = 0.1;
    config::save(&path, &cfg).unwrap();
    assert_eq!(config::load(&path).unwrap(), cfg);
    assert!(!dir.join("config.tmp").exists());
    cfg.axes.roll.deadzone = 3.0;
    assert!(config::save(&path, &cfg).unwrap_err().contains("Roll deadzone"));
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
    assert_eq!(shared.config(), (1, cfg.clone()));
    let mut bad = config::defaults();
    bad.version = 0;
    assert!(shared.save(bad.clone()).is_err());
    assert_eq!(shared.generation(), 1);
    // A preview replaces the active settings without touching the saved profile; a save or a clear ends it.
    let mut draft = cfg.clone();
    draft.axes.yaw.curve = 0.4;
    assert!(shared.set_preview(Some(bad)).is_err());
    assert!(!shared.previewing());
    shared.set_preview(Some(draft.clone())).unwrap();
    assert_eq!((shared.active(), shared.config().1.clone(), shared.previewing()), ((2, draft.clone()), cfg.clone(), true));
    shared.set_preview(None).unwrap();
    assert_eq!((shared.active(), shared.previewing()), ((3, cfg.clone()), false));
    shared.set_preview(Some(draft.clone())).unwrap();
    shared.save(draft.clone()).unwrap();
    assert_eq!((shared.active(), shared.previewing()), ((5, draft), false));
    assert_eq!(config::load(&dir.join("config.json")).unwrap().axes.yaw.curve, 0.4);
    assert!(!shared.paused());
    shared.set_paused(true);
    assert!(shared.paused());
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
