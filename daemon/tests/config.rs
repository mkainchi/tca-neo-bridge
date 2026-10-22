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

fn target(buttons: &[u8]) -> config::Target {
    config::Target { buttons: buttons.to_vec(), pov: None }
}

#[test]
fn gestures_layers_camera_and_yaw_round_trip_and_default_when_absent() {
    let mut cfg = with_tca();
    let p = tca(&mut cfg);
    p.long_press.insert("tca-stick/b2".into(), target(&[3]));
    p.double_tap.insert("tca-stick/b2".into(), config::Target { buttons: vec![], pov: Some(Direction::Left) });
    p.shift = Some("tca-stick/b16".into());
    p.shifted.buttons.insert("tca-stick/b1".into(), vec![9]);
    p.shifted.pov.insert("tca-stick/b3".into(), Direction::Down);
    p.behaviours.insert("tca-stick/b7".into(), config::Mode::Repeat);
    p.yaw_source = config::YawSource::Off;
    cfg.camera = config::Camera { ramp_ms: 300, full_after_hold: true };
    assert_eq!(config::parse(&config::to_json(&cfg)).unwrap(), cfg);
    // A camera lever on a split lever needs the split rule off; lever yaw too.
    tca(&mut cfg).camera_lever = Some(Binding::new("tca-quadrant", "right", true));
    assert!(config::validate(&cfg).unwrap_err().contains("turn on only one"));
    tca(&mut cfg).split_high_g.enabled = false;
    assert!(config::validate(&cfg).is_ok());
    tca(&mut cfg).camera_lever = Some(Binding::new("tca-stick", "slider", false)); // a lever of its own
    tca(&mut cfg).yaw_source = config::YawSource::LeverDifferential;
    assert!(config::validate(&cfg).is_ok());
    // A version 3 profile without the later settings: they default.
    let mut value: serde_json::Value = serde_json::from_str(&config::to_json(&with_tca())).unwrap();
    let object = value.as_object_mut().unwrap();
    for key in ["throttle", "safe_start", "timing", "camera", "head_tracking"] {
        object.remove(key);
    }
    let profile = object["setups"][config::TCA_SETUP].as_object_mut().unwrap();
    for key in ["camera_lever", "yaw_source", "precision", "combos", "behaviours", "long_press", "double_tap", "shift", "shifted"] {
        profile.remove(key);
    }
    let early = config::parse(&value.to_string()).unwrap();
    assert!(early.safe_start);
    assert_eq!((early.timing, early.camera, early.head_tracking), (config::Timing::default(), config::Camera::default(), config::HeadTracking::default()));
    let p = &early.setups[config::TCA_SETUP];
    assert_eq!((p.shift.clone(), p.yaw_source, p.behaviours.len(), p.shifted.clone()), (None, config::YawSource::Twist, 0, config::Layer::default()));
    // Timing saved before long press, double tap and repeat existed keeps its pulse and gains the defaults.
    let timing: config::Timing = serde_json::from_str(r#"{"pulse_ms": 60}"#).unwrap();
    assert_eq!(timing, config::Timing { pulse_ms: 60, long_ms: 500, double_ms: 250, repeat_ms: 120 });
}

#[test]
fn validation_rules() {
    assert!(config::validate(&config::defaults()).is_ok());
    assert!(config::validate(&with_tca()).is_ok());
    let cases: Vec<(Change, &str)> = vec![
        (Box::new(|c| c.version = 2), "version"),
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
        (Box::new(|c| c.timing.pulse_ms = 10), "Pulse length"),
        (Box::new(|c| c.timing.pulse_ms = 2000), "Pulse length"),
        (Box::new(|c| c.timing.long_ms = 100), "long press needs"),
        (Box::new(|c| c.timing.long_ms = 3000), "long press needs"),
        (Box::new(|c| c.timing.double_ms = 50), "long press needs"),
        (Box::new(|c| c.timing.double_ms = 1500), "long press needs"),
        (Box::new(|c| c.timing.repeat_ms = 20), "Repeat interval"),
        (Box::new(|c| c.timing.repeat_ms = 2000), "Repeat interval"),
        (Box::new(|c| c.timing.repeat_ms = 80), "Repeat interval"),
        (Box::new(|c| c.camera.ramp_ms = 2500), "Camera ramp"),
        (Box::new(|c| c.head_tracking.port = 80), "Head tracking needs"),
        (Box::new(|c| c.head_tracking.yaw_range = 2.0), "Head tracking needs"),
        (Box::new(|c| c.head_tracking.yaw_range = 200.0), "Head tracking needs"),
        (Box::new(|c| c.head_tracking.pitch_range = 2.0), "Head tracking needs"),
        (Box::new(|c| c.head_tracking.pitch_range = 100.0), "Head tracking needs"),
        (Box::new(|c| c.head_tracking.timeout_ms = 50), "Head tracking needs"),
        (Box::new(|c| c.head_tracking.timeout_ms = 9000), "Head tracking needs"),
        (Box::new(|c| c.head_tracking.tuning.curve = 2.0), "Head tracking curve"),
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
        (Box::new(|c| { tca(c).split_high_g = config::SplitHighG::default(); tca(c).yaw_source = config::YawSource::LeverDifferential; }), "exactly two levers"),
        (Box::new(|c| tca(c).bindings.throttle = vec![Binding::new("x", "z", false); 9]), "eight throttle"),
        (Box::new(|c| tca(c).bindings.roll = Some(Binding::new("X", "x", false))), "Unknown axis X/x"),
        (Box::new(|c| tca(c).bindings.throttle.push(Binding::new("x", "", false))), "Unknown axis x/"),
        (Box::new(|c| tca(c).split_high_g.levers[1].axis = "a".repeat(17)), "Unknown axis"),
        (Box::new(|c| tca(c).bindings.yaw = Some(Binding::new("x", "Z", false))), "Unknown axis"),
        (Box::new(|c| tca(c).camera_lever = Some(Binding::new("x", "Z", false))), "Unknown axis"),
        (Box::new(|c| tca(c).camera_hat = Some("#".into())), "camera hat"),
        (Box::new(|c| tca(c).yaw_source = config::YawSource::LeverDifferential), "turn on only one"),
        (Box::new(|c| tca(c).camera_lever = Some(Binding::new("tca-quadrant", "left", true))), "turn on only one"),
        (Box::new(|c| {
            tca(c).split_high_g.enabled = false;
            tca(c).camera_lever = Some(Binding::new("tca-quadrant", "right", false));
            tca(c).yaw_source = config::YawSource::LeverDifferential;
        }), "turn on only one"),
        (Box::new(|c| { tca(c).buttons.insert("z9".into(), vec![1]); }), "Unknown control z9"),
        (Box::new(|c| { tca(c).buttons.insert("tca-stick/b3".into(), vec![16]); }), "tca-stick/b3: NEO buttons"),
        (Box::new(|c| { tca(c).buttons.insert("tca-stick/b3".into(), vec![0]); }), "tca-stick/b3: NEO buttons"),
        (Box::new(|c| { tca(c).pov.insert("q40".into(), Direction::Up); }), "Unknown control q40"),
        (Box::new(|c| tca(c).precision.source = Some("s40".into())), "Unknown control s40"),
        (Box::new(|c| tca(c).precision.scale = 0.05), "Precision scale"),
        (Box::new(|c| tca(c).combos.pause = (1..=5).map(|n| format!("tca-stick/b{n}")).collect()), "A combo has at most 4"),
        (Box::new(|c| tca(c).combos.next_profile = vec!["q99".into()]), "Unknown control q99"),
        (Box::new(|c| { tca(c).behaviours.insert("x".into(), config::Mode::Toggle); }), "Unknown control x"),
        (Box::new(|c| { tca(c).long_press.insert("tca-stick/b129".into(), target(&[1])); }), "Unknown control tca-stick/b129"),
        (Box::new(|c| { tca(c).long_press.insert("tca-stick/b2".into(), target(&[16])); }), "tca-stick/b2: NEO buttons"),
        (Box::new(|c| { tca(c).double_tap.insert("tca-stick/b2".into(), target(&[0])); }), "tca-stick/b2: NEO buttons"),
        (Box::new(|c| { tca(c).shifted.buttons.insert("tca-quadrant/b1".into(), vec![20]); }), "tca-quadrant/b1: NEO buttons"),
        (Box::new(|c| { tca(c).shifted.pov.insert("nope".into(), Direction::Up); }), "Unknown control nope"),
        (Box::new(|c| tca(c).shift = Some("s0".into())), "Unknown control s0"),
    ];
    for (change, message) in cases {
        let mut cfg = with_tca();
        change(&mut cfg);
        assert!(config::validate(&cfg).unwrap_err().contains(message), "{message}");
    }
    // Split rule off with no levers, a camera on any valid id and axis names with digits are fine.
    let mut cfg = with_tca();
    tca(&mut cfg).bindings.throttle.push(Binding::new("x", "slider2", false));
    tca(&mut cfg).split_high_g = config::SplitHighG::default();
    tca(&mut cfg).camera_hat = None;
    assert!(config::validate(&cfg).is_ok());
}

#[test]
fn parse_accepts_v3_and_migrates_the_experimental_v2() {
    let text = config::to_json(&with_tca());
    assert_eq!(config::parse(&text).unwrap(), with_tca());
    assert!(config::parse("{").unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse("[]").unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&text.replacen('{', "{\"extra\": 1,", 1)).unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&text.replace("\"version\": 3", "\"version\": 4")).unwrap_err().contains("version"));
    assert_eq!(config::migrate(serde_json::json!([1])).unwrap(), serde_json::json!([1]));
    // The experimental v2: one deadzone for every axis, axes and camera inside each setup; optional fields default.
    let minimal = r#"{"version": 2, "deadzone": 0.1, "camera_strength": 0.5,
        "setups": {"x": {"buttons": {}, "pov": {}, "axes": {"roll": {"device": "x", "axis": "x"}}, "camera": "x"}, "y": {"buttons": {}, "pov": {}}}}"#;
    let parsed = config::parse(minimal).unwrap();
    assert!(parsed.devices.is_empty());
    let axis = config::AxisTuning { deadzone: 0.1, ..config::AxisTuning::default() };
    assert_eq!(parsed.axes, config::Tuning { roll: axis, pitch: axis, yaw: axis });
    assert_eq!((parsed.setups["x"].bindings.roll.clone(), parsed.setups["x"].camera_hat.as_deref()), (Some(Binding::new("x", "x", false)), Some("x")));
    assert_eq!((parsed.setups["y"].hat, parsed.setups["y"].split_high_g.clone()), (config::HatMode::Camera, config::SplitHighG::default()));
    let bare = config::parse(r#"{"version": 2, "deadzone": 0.04, "camera_strength": 1.0}"#).unwrap();
    assert!(bare.setups.is_empty());
}

/// A profile exactly as version 0.2.2 saved it.
const V1: &str = r#"{"version": 1, "deadzone": 0.07, "camera_strength": 0.5, "throttle_lever": "right", "hat": "mapped",
    "split_high_g": {"enabled": false, "forward": 0.8, "back": 0.2}, "invert": {"roll": false, "pitch": true, "yaw": false, "throttle": false},
    "buttons": {"q3": [6], "s1": [1], "hat-left": [3]}, "pov": {"s11": "up"}}"#;

#[test]
fn version_1_profiles_migrate_to_one_tca_setup_and_keep_holding() {
    let cfg = config::parse(V1).unwrap();
    assert_eq!(cfg.version, config::VERSION);
    let axis = config::AxisTuning { deadzone: 0.07, ..config::AxisTuning::default() };
    assert_eq!(cfg.axes, config::Tuning { roll: axis, pitch: axis, yaw: axis });
    assert!(cfg.safe_start);
    assert_eq!((cfg.camera_strength, cfg.throttle, cfg.timing), (0.5, config::Throttle::default(), config::Timing::default()));
    let p = &cfg.setups[config::TCA_SETUP];
    assert!(p.behaviours.is_empty()); // the latching switches keep holding, as before
    assert_eq!(p.buttons.keys().collect::<Vec<_>>(), ["tca-quadrant/b3", "tca-stick/b1", "tca-stick/hat-left"]);
    assert_eq!(p.pov["tca-stick/b11"], Direction::Up);
    assert_eq!((p.hat, p.camera_hat.as_deref(), p.camera_lever.clone()), (config::HatMode::Mapped, Some("tca-stick"), None));
    assert_eq!((p.bindings.roll.clone(), p.bindings.pitch.clone()), (Some(Binding::new("tca-stick", "x", false)), Some(Binding::new("tca-stick", "y", true))));
    assert_eq!(p.bindings.throttle, [Binding::new("tca-stick", "slider", false), Binding::new("tca-quadrant", "right", false)]);
    assert_eq!((p.split_high_g.enabled, p.split_high_g.forward, p.split_high_g.levers.len()), (false, 0.8, 2));
    // Saved before the split and hat settings existed: split on at 75% / 25%, the hat on the camera.
    let early = config::parse(&V1.replace(r#""hat": "mapped","#, "").replace(r#""split_high_g": {"enabled": false, "forward": 0.8, "back": 0.2}, "#, "")).unwrap();
    let p = &early.setups[config::TCA_SETUP];
    assert_eq!((p.hat, p.split_high_g.enabled, p.split_high_g.forward, p.split_high_g.back), (config::HatMode::Camera, true, 0.75, 0.25));
    // A v1 deadzone out of range is reported, not clamped; unknown keys, a missing deadzone or a bad lever are refused.
    assert!(config::parse(&V1.replace("0.07", "0.8")).unwrap_err().contains("Roll deadzone"));
    assert!(config::parse(&V1.replace("\"deadzone\": 0.07,", "")).unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&V1.replace("\"q3\"", "\"q32\"")).unwrap_err().contains("Unknown TCA control q32"));
    assert!(config::parse(&V1.replace("\"s11\"", "\"z9\"")).unwrap_err().contains("Unknown TCA control z9"));
    assert!(config::parse(&V1.replace("\"right\"", "\"middle\"")).unwrap_err().starts_with("Invalid profile"));
    assert!(config::parse(&V1.replace("\"split_high_g\": {\"enabled\": false, \"forward\": 0.8, \"back\": 0.2}", "\"split_high_g\": 5")).unwrap_err().starts_with("Invalid profile"));
    assert_eq!(config::migrate_key("hat-up-right"), None);
    for bad in ["s", "s0", "s01", "s18", "q32", "s1a", "x1", "", "s99999999999999999999999"] {
        assert_eq!(config::migrate_key(bad), None, "{bad}");
    }
}

/// A profile as the stable bridge 0.3.0 saves it, with every phase setting in use.
const STABLE: &str = r#"{"version": 2,
    "axes": {"roll": {"deadzone": 0.05, "saturation": 0.9, "anti_deadzone": 0.1, "curve": 0.3, "smoothing": 0.2},
             "pitch": {"deadzone": 0.04, "saturation": 1.0, "anti_deadzone": 0.0, "curve": 0.0, "smoothing": 0.0},
             "yaw": {"deadzone": 0.04, "saturation": 1.0, "anti_deadzone": 0.0, "curve": -0.2, "smoothing": 0.0}},
    "camera_strength": 0.8, "throttle_lever": "right", "hat": "camera",
    "split_high_g": {"enabled": false, "forward": 0.75, "back": 0.25},
    "invert": {"roll": false, "pitch": true, "yaw": false, "throttle": true},
    "throttle": {"mode": "detents", "idle": 0.1, "cruise": 0.5, "boost": 0.9, "band": 0.03},
    "precision": {"source": "s4", "scale": 0.4}, "safe_start": false,
    "combos": {"pause": ["s5", "q6"], "next_profile": ["s16"]},
    "behaviours": {"q3": "pulse_change"}, "timing": {"pulse_ms": 80, "long_ms": 600, "double_ms": 250, "repeat_ms": 120},
    "long_press": {"s2": {"buttons": [3], "pov": null}}, "double_tap": {"s3": {"buttons": [], "pov": "left"}},
    "shift": "s17", "shifted": {"buttons": {"s1": [9]}, "pov": {"s2": "down"}},
    "camera": {"ramp_ms": 300, "full_after_hold": true, "lever": true}, "yaw_source": "twist",
    "head_tracking": {"enabled": true, "port": 5000, "yaw_range": 45.0, "pitch_range": 30.0, "invert_yaw": false, "invert_pitch": false, "timeout_ms": 500,
                      "tuning": {"deadzone": 0.03, "saturation": 1.0, "anti_deadzone": 0.0, "curve": 0.0, "smoothing": 0.0}},
    "buttons": {"s1": [1], "q3": [6]}, "pov": {"s11": "up"}}"#;

#[test]
fn stable_version_2_profiles_migrate_with_every_setting() {
    let cfg = config::parse(STABLE).unwrap();
    assert_eq!((cfg.axes.roll.curve, cfg.axes.yaw.curve, cfg.camera_strength), (0.3, -0.2, 0.8));
    assert_eq!((cfg.throttle.mode, cfg.safe_start, cfg.timing.long_ms, cfg.head_tracking.port), (config::ThrottleMode::Detents, false, 600, 5000));
    assert_eq!(cfg.camera, config::Camera { ramp_ms: 300, full_after_hold: true });
    let p = &cfg.setups[config::TCA_SETUP];
    assert_eq!(p.precision, config::Precision { source: Some("tca-stick/b4".into()), scale: 0.4 });
    assert_eq!((p.combos.pause.clone(), p.combos.next_profile.clone()), (vec!["tca-stick/b5".to_string(), "tca-quadrant/b6".into()], vec!["tca-stick/b16".to_string()]));
    assert_eq!(p.behaviours["tca-quadrant/b3"], config::Mode::PulseChange);
    assert_eq!((p.long_press["tca-stick/b2"].buttons.clone(), p.double_tap["tca-stick/b3"].pov), (vec![3], Some(Direction::Left)));
    assert_eq!((p.shift.as_deref(), p.shifted.buttons["tca-stick/b1"].clone(), p.shifted.pov["tca-stick/b2"]), (Some("tca-stick/b17"), vec![9], Direction::Down));
    // The camera lever was the quadrant lever that is not the throttle (left here), read the other way round.
    assert_eq!(p.camera_lever, Some(Binding::new("tca-quadrant", "left", true)));
    assert_eq!(p.bindings.throttle, [Binding::new("tca-stick", "slider", true), Binding::new("tca-quadrant", "right", true)]);
    assert_eq!((p.bindings.pitch.clone(), p.split_high_g.levers[0].clone()), (Some(Binding::new("tca-stick", "y", true)), Binding::new("tca-quadrant", "left", true)));
    assert_eq!((p.buttons.len(), p.pov["tca-stick/b11"]), (2, Direction::Up));
    // The least a stable profile needs; no camera lever on the left-lever throttle names the right lever.
    let least = r#"{"version": 2, "axes": {"roll": {"deadzone": 0.04, "saturation": 1.0, "anti_deadzone": 0.0, "curve": 0.0, "smoothing": 0.0},
        "pitch": {"deadzone": 0.04, "saturation": 1.0, "anti_deadzone": 0.0, "curve": 0.0, "smoothing": 0.0},
        "yaw": {"deadzone": 0.04, "saturation": 1.0, "anti_deadzone": 0.0, "curve": 0.0, "smoothing": 0.0}},
        "camera_strength": 1.0, "throttle_lever": "left", "precision": {"source": null, "scale": 0.5},
        "camera": {"ramp_ms": 0, "full_after_hold": false, "lever": false}, "buttons": {}, "pov": {}}"#;
    let cfg = config::parse(least).unwrap();
    let p = &cfg.setups[config::TCA_SETUP];
    assert_eq!((p.precision.source.clone(), p.camera_lever.clone(), p.split_high_g.enabled), (None, None, true));
    assert_eq!(p.bindings.roll, Some(Binding::new("tca-stick", "x", false)));
    let spare = STABLE.replace(r#""throttle_lever": "right""#, r#""throttle_lever": "left""#);
    assert_eq!(config::parse(&spare).unwrap().setups[config::TCA_SETUP].camera_lever, Some(Binding::new("tca-quadrant", "right", true)));
    assert!(config::parse(&STABLE.replace(r#""shift": "s17""#, r#""shift": "s18""#)).unwrap_err().contains("Unknown TCA control s18"));
    assert!(config::parse(&STABLE.replace(r#""shifted": {"buttons": {"s1": [9]}"#, r#""shifted": {"buttons": {"x": [9]}"#)).unwrap_err().contains("Unknown TCA control x"));
    assert!(config::parse(&STABLE.replace(r#""pause": ["s5""#, r#""pause": ["s50""#)).unwrap_err().contains("Unknown TCA control s50"));
    assert!(config::parse(&STABLE.replace(r#""source": "s4""#, r#""source": "s0""#)).unwrap_err().contains("Unknown TCA control s0"));
    for (from, to) in [(r#"{"q3": "pulse_change"}"#, r#"{"q99": "pulse_change"}"#), (r#""long_press": {"s2""#, r#""long_press": {"s99""#), (r#""double_tap": {"s3""#, r#""double_tap": {"x""#), (r#""pov": {"s2": "down"}"#, r#""pov": {"z": "down"}"#), (r#""next_profile": ["s16"]"#, r#""next_profile": ["s99"]"#)] {
        assert!(config::parse(&STABLE.replace(from, to)).unwrap_err().contains("Unknown TCA control"), "{to}");
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
    assert_eq!(alone.bindings.throttle, [Binding::new("tca-stick", "slider", false)]);
    assert!(!alone.split_high_g.enabled);
}

#[test]
fn defaults_pulse_the_latching_switches_and_arm_a_safe_start() {
    let cfg = config::defaults();
    assert!(cfg.safe_start && cfg.setups.is_empty() && cfg.devices.is_empty());
    assert_eq!((cfg.throttle.mode, cfg.timing.pulse_ms), (config::ThrottleMode::Linear, 80));
    let p = config::tca_profile();
    let modes: Vec<_> = p.behaviours.iter().map(|(k, m)| (k.as_str(), *m)).collect();
    use config::Mode::{Pulse, PulseChange};
    assert_eq!(modes, [("tca-quadrant/b3", PulseChange), ("tca-quadrant/b4", PulseChange), ("tca-quadrant/b7", Pulse), ("tca-quadrant/b8", Pulse)]);
    assert!(p.precision.source.is_none() && p.combos.pause.is_empty());
    let mut valid = with_tca();
    tca(&mut valid).precision.source = Some("tca-stick/hat-down".into());
    tca(&mut valid).combos.pause = vec!["tca-stick/b5".into(), "tca-quadrant/b6".into()];
    assert_eq!(config::parse(&config::to_json(&valid)).unwrap(), valid);
}

#[test]
fn load_save_round_trip_and_failures() {
    let dir = temp("roundtrip");
    let path = dir.join("config.json");
    assert_eq!(config::load(&path).unwrap(), config::defaults());
    let mut cfg = with_tca();
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
fn shared_state_publishes_generations_status_and_devices() {
    let dir = temp("shared");
    let shared = Shared::new(dir.join("config.json"), config::defaults());
    assert_eq!(shared.generation(), 0);
    assert_eq!(shared.present(), Ok(Vec::new())); // no scanner, no session
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
    *shared.devices.lock().unwrap() = Some(vec![models::tca_stick()]);
    assert_eq!(shared.present().unwrap()[0].id, "tca-stick"); // the session's devices win
}
