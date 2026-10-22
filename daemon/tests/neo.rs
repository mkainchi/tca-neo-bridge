use neo_daemon::config::{self, Binding, Direction, Mode, ThrottleMode};
use neo_daemon::hid::{self, InputState};
use neo_daemon::models::{self, Axis, DeviceInfo, Layout};
use neo_daemon::neo::{self, input_report, unit16, Event, Mapper, ThrottleSelector};
use neo_daemon::{protocol, validation};
use std::time::Duration;

fn stick(x: u16, y: u16, yaw: u8, slider: u8, hat: u8, buttons: &[u32]) -> Vec<u8> {
    let mut report = vec![0u8; 64];
    report[0] = 1;
    report[1..3].copy_from_slice(&x.to_le_bytes());
    report[3..5].copy_from_slice(&y.to_le_bytes());
    report[5..8].copy_from_slice(&[yaw, slider, hat]);
    let mask: u32 = buttons.iter().map(|b| 1 << (b - 1)).sum();
    report[8..11].copy_from_slice(&mask.to_le_bytes()[..3]);
    report
}

fn neutral_stick() -> Vec<u8> {
    stick(8192, 8192, 128, 128, 8, &[])
}

fn quadrant(left: u16, right: u16, buttons: &[u32]) -> Vec<u8> {
    let mut report = vec![0u8; 44];
    report[0] = 1;
    report[3..5].copy_from_slice(&left.to_le_bytes());
    report[5..7].copy_from_slice(&right.to_le_bytes());
    let mask: u32 = buttons.iter().map(|b| 1 << (b - 1)).sum();
    report[1..3].copy_from_slice(&((mask & 0xffff) as u16).to_le_bytes());
    report[9] = (mask >> 16 & 15) as u8;
    report[12..14].copy_from_slice(&((mask >> 20) as u16).to_le_bytes());
    report
}

fn devices() -> Vec<DeviceInfo> {
    vec![models::tca_stick(), models::tca_quadrant()]
}

/// Decoded TCA pair, as the readers publish it.
fn tca(stick: &[u8], quadrant: &[u8]) -> Vec<Option<InputState>> {
    vec![hid::tca_stick(stick), hid::tca_quadrant(quadrant)]
}

fn convert(mapper: &mut Mapper, stick: &[u8], quadrant: &[u8]) -> neo::Report {
    mapper.convert(&tca(stick, quadrant))
}

fn fields(report: &neo::Report) -> ([u16; 7], u32, u8) {
    let axes = std::array::from_fn(|i| u16::from_le_bytes([report[4 + 2 * i], report[5 + 2 * i]]));
    let packed = u32::from_le_bytes([report[1], report[2], report[3], 0]);
    (axes, packed >> 4, report[1] & 15)
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

/// A version 1 (TCA-only) profile with `changes` applied, migrated: the vectors below prove migration keeps output.
fn v1(changes: serde_json::Value) -> config::Config {
    let mut value = serde_json::json!({
        "version": 1, "deadzone": 0.04, "camera_strength": 1.0, "throttle_lever": "left", "hat": "camera",
        "split_high_g": {"enabled": true, "forward": 0.75, "back": 0.25},
        "invert": {"roll": false, "pitch": false, "yaw": false, "throttle": false},
        "buttons": {}, "pov": {}
    });
    for (key, change) in changes.as_object().unwrap() {
        value[key] = change.clone();
    }
    config::parse(&value.to_string()).unwrap()
}

/// The research mapping from tca_to_neo.py (DEFAULT_BINDINGS / ORDERS), which produced the Python vectors,
/// with the settings these tests were written against: throttle not inverted, split High-G at 95% / 5%.
fn research_json() -> serde_json::Value {
    serde_json::json!({
        "split_high_g": {"enabled": true, "forward": 0.95, "back": 0.05},
        "buttons": {"s1": [1], "s2": [2], "s3": [3], "s4": [4], "s5": [5], "s6": [7], "s7": [8], "s8": [9], "s9": [1], "s10": [12], "s16": [4], "q1": [2], "q2": [7]},
        "pov": {"s11": "up", "s12": "down", "s13": "left", "s14": "right"}
    })
}

fn research() -> config::Config {
    // The research converter held every source and had no safe start.
    let mut cfg = v1(research_json());
    cfg.safe_start = false;
    cfg
}

fn research_with(extra: serde_json::Value) -> config::Config {
    let mut value = research_json();
    for (key, change) in extra.as_object().unwrap() {
        value[key] = change.clone();
    }
    let mut cfg = v1(value);
    cfg.safe_start = false;
    cfg
}

fn profile(cfg: &mut config::Config) -> &mut config::Profile {
    cfg.setups.get_mut(config::TCA_SETUP).unwrap()
}

fn mapper() -> Mapper {
    Mapper::new(&research(), &devices())
}

/// Roll, pitch and yaw passed through untouched.
fn linear() -> config::Tuning {
    let axis = config::AxisTuning { deadzone: 0.0, ..config::AxisTuning::default() };
    config::Tuning { roll: axis, pitch: axis, yaw: axis }
}

fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

/// Lever raw value for a travel fraction (raw 65535 idle, 0 fully forward).
fn raw(travel: f64) -> u16 {
    (65535.0 * (1.0 - travel)).round() as u16
}

#[test]
fn python_research_vectors_match_byte_for_byte() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/tca-neo-mapping-vectors.json");
    let vectors: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let vectors = vectors.as_array().unwrap();
    assert_eq!(vectors.len(), 8);
    for vector in vectors {
        let mut mapper = mapper();
        let stick = hex(vector["synthetic_stick_report"].as_str().unwrap());
        let quadrant = hex(vector["synthetic_quadrant_report"].as_str().unwrap());
        let report = convert(&mut mapper, &stick, &quadrant);
        assert_eq!(report.to_vec(), hex(vector["neo_report"].as_str().unwrap()), "{}", vector["name"]);
    }
}

#[test]
fn default_tca_profile_maps_sidestick_numbers_and_recorded_quadrant_controls() {
    let mut mapper = Mapper::new(&config::defaults(), &devices());
    let neutral_quadrant = quadrant(32768, 32768, &[]);
    for n in 1..=15u32 {
        let (_, buttons, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 8, &[n]), &neutral_quadrant));
        assert_eq!((buttons, hat), (1 << (n - 1), 8), "sidestick {n}");
    }
    // Pulsed sources (ENG switches, mode selector) fire when they switch on after the first frame.
    for (button, neo) in [(1, 5), (2, 5), (3, 6), (4, 6), (5, 9), (6, 6), (7, 7), (8, 8)] {
        let mut mapper = Mapper::new(&config::defaults(), &devices());
        convert(&mut mapper, &neutral_stick(), &neutral_quadrant);
        let (_, buttons, _) = fields(&convert(&mut mapper, &neutral_stick(), &quadrant(32768, 32768, &[button])));
        assert_eq!(buttons, 1 << (neo - 1), "quadrant {button}");
    }
    // Tuned settings: no axis inverted (checked in AC8), split High-G at 75% / 25%.
    let tca = config::default_profile(&devices());
    assert_eq!(tca, config::tca_profile());
    assert!(!tca.bindings.throttle.iter().chain(&tca.split_high_g.levers).any(|b| b.invert));
    assert_eq!((tca.split_high_g.enabled, tca.split_high_g.forward, tca.split_high_g.back), (true, 0.75, 0.25));
    // Sidestick 16/17 and the remaining quadrant inputs are unmapped; no control sends the POV; the hat is camera.
    let (axes, buttons, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 0, &[16, 17]), &quadrant(32768, 32768, &[9, 31])));
    assert_eq!((buttons, hat, axes[4]), (0, 8, 0));
    assert!(tca.pov.is_empty());
    // The v1 default profile migrates to the TCA default, holding every source as v1 did (no behaviours).
    let migrated = v1(serde_json::json!({"buttons": {"s1": [1], "s2": [2], "s3": [3], "s4": [4], "s5": [5], "s6": [6], "s7": [7], "s8": [8], "s9": [9], "s10": [10], "s11": [11], "s12": [12], "s13": [13], "s14": [14], "s15": [15], "q1": [5], "q2": [5], "q3": [6], "q4": [6], "q5": [9], "q6": [6], "q7": [7], "q8": [8]}}));
    assert_eq!(migrated.setups[config::TCA_SETUP], config::Profile { behaviours: Default::default(), ..config::tca_profile() });
}

#[test]
fn split_levers_hold_high_g_and_the_throttle_follows_the_forward_lever() {
    // The TCA reports a lever fully forward as raw 0 and at idle as raw 65535.
    let neutral = neutral_stick();
    let (forward_raw, idle_raw) = (0u16, 65535u16);
    let mut mapper = mapper();
    let normal = |mapper: &mut Mapper, raw: u16| fields(&convert(mapper, &neutral, &quadrant(raw, raw, &[]))).0[2];
    // Left forward, right back: High-G (NEO 5) held, throttle as if the forward lever alone were the throttle.
    let (axes, buttons, _) = fields(&convert(&mut mapper, &neutral, &quadrant(forward_raw, idle_raw, &[])));
    assert_eq!((axes[2], buttons), (0, 1 << 4));
    // Either order; thresholds are inclusive (95% / 5% of travel).
    let forward = (0.05f64 * 65535.0).floor() as u16;
    let back = (0.95f64 * 65535.0).ceil() as u16;
    let (axes, buttons, _) = fields(&convert(&mut mapper, &neutral, &quadrant(back, forward, &[])));
    assert_eq!((axes[2], buttons), (normal(&mut mapper, forward), 1 << 4));
    // Just outside either threshold: no High-G.
    assert_eq!(fields(&convert(&mut mapper, &neutral, &quadrant(forward + 2, idle_raw, &[]))).1, 0);
    assert_eq!(fields(&convert(&mut mapper, &neutral, &quadrant(forward_raw, back - 2, &[]))).1, 0);
    // Throttle inverted, thresholds 75% / 25%: the split throttle still follows the forward lever and is inverted
    // exactly like the normal throttle, never the back lever's idle.
    let cfg = research_with(serde_json::json!({"invert": {"roll": false, "pitch": false, "yaw": false, "throttle": true}, "split_high_g": {"enabled": true, "forward": 0.75, "back": 0.25}}));
    let mut inverted = Mapper::new(&cfg, &devices());
    let idle = normal(&mut inverted, idle_raw);
    let full_normal = normal(&mut inverted, forward_raw);
    assert_eq!((idle, full_normal), (0, 65535));
    for levers in [(forward_raw, idle_raw), (idle_raw, forward_raw)] {
        let (axes, buttons, _) = fields(&convert(&mut inverted, &neutral, &quadrant(levers.0, levers.1, &[])));
        assert_eq!((axes[2], buttons), (full_normal, 1 << 4), "{levers:?}");
    }
    let (axes, buttons, _) = fields(&convert(&mut inverted, &neutral, &quadrant(16383, 49152, &[])));
    assert_eq!((axes[2], buttons), (normal(&mut inverted, 16383), 1 << 4)); // 75% / 25%: inclusive
    // Disabled: levers act normally.
    let cfg = research_with(serde_json::json!({"split_high_g": {"enabled": false, "forward": 0.95, "back": 0.05}}));
    assert_eq!(fields(&convert(&mut Mapper::new(&cfg, &devices()), &neutral, &quadrant(forward_raw, idle_raw, &[]))).1, 0);
    // Combined with a mapped High-G button the bit is simply set once.
    let (_, buttons, _) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 8, &[5]), &quadrant(idle_raw, forward_raw, &[])));
    assert_eq!(buttons, 1 << 4);
}

#[test]
fn reversed_levers_count_travel_from_the_low_raw_end() {
    let mut cfg = research();
    profile(&mut cfg).split_high_g.reverse = true;
    let mut mapper = Mapper::new(&cfg, &devices());
    // Raw 65535 is now fully forward: left forward, right back.
    let inputs = tca(&neutral_stick(), &quadrant(65535, 0, &[]));
    assert_eq!(mapper.levers(&inputs), Some([1.0, 0.0]));
    let (axes, buttons, _) = fields(&mapper.convert(&inputs));
    assert_eq!((axes[2], buttons), (65535, 1 << 4)); // throttle follows the forward lever's raw value
}

#[test]
fn lever_positions_and_split_rule() {
    // Travel: raw 65535 is idle (0.0), raw 0 is fully forward (1.0).
    let mapper = mapper();
    assert_eq!(mapper.levers(&tca(&neutral_stick(), &quadrant(65535, 0, &[]))), Some([0.0, 1.0]));
    assert_eq!(mapper.levers(&[hid::tca_stick(&neutral_stick()), None]), None);
    let mut unbound = research();
    profile(&mut unbound).split_high_g.levers.clear();
    profile(&mut unbound).split_high_g.enabled = false;
    assert_eq!(Mapper::new(&unbound, &devices()).levers(&tca(&neutral_stick(), &quadrant(65535, 0, &[]))), None);
    // Levers on two devices: both must report; inputs shorter than the device list count as missing.
    let mut spread = research();
    profile(&mut spread).split_high_g.levers = vec![Binding::new("tca-stick", "slider", false), Binding::new("tca-quadrant", "left", false)];
    let spread = Mapper::new(&spread, &devices());
    assert_eq!(spread.levers(&[hid::tca_stick(&neutral_stick()), None]), None);
    assert_eq!(spread.levers(&tca(&neutral_stick(), &quadrant(0, 0, &[]))[..1]), None);
    // Bindings and keys naming absent devices or nonsense are ignored by the mapper.
    let mut odd = research();
    profile(&mut odd).bindings.roll = Some(Binding::new("absent", "x", false));
    profile(&mut odd).buttons.insert("junk".into(), vec![1]);
    let (axes, buttons, _) = fields(&convert(&mut Mapper::new(&odd, &devices()), &stick(0, 8192, 128, 128, 8, &[]), &quadrant(32768, 32768, &[])));
    assert_eq!((axes[0], buttons), (32768, 0));
    let rule = config::tca_profile().split_high_g;
    assert!(neo::split(&rule, [0.0, 1.0]));
    assert!(!neo::split(&rule, [0.5, 1.0]));
    assert!(!neo::split(&config::SplitHighG { enabled: false, ..rule }, [0.0, 1.0]));
}

#[test]
fn input_report_layout_and_limits() {
    let report = input_report([1, 2, 3, 4, 5, 6, 0xffff], 0x7fff, 3);
    assert_eq!(report[0], 1);
    assert_eq!(fields(&report), ([1, 2, 3, 4, 5, 6, 0xffff], 0x7fff, 3));
    assert_eq!(fields(&input_report([0; 7], 0xffff, 200)), ([0; 7], 0x7fff, 8));
    assert_eq!(fields(&neo::neutral()), ([32768; 7], 0, 8));
    assert!(report[18..].iter().all(|&b| b == 0));
}

#[test]
fn centered_unit16_and_ties_to_even() {
    assert_eq!(unit16(0.0), 32768); // 32767.5 rounds to even
    assert_eq!(unit16(-5.0), 0);
    assert_eq!(unit16(5.0), 65535);
}

#[test]
fn throttle_last_meaningful_move_wins() {
    let mut selector = ThrottleSelector::default();
    assert_eq!(selector.read(&[]), None);
    assert_eq!(selector.read(&[0.5, -0.5]), Some((1, -0.5))); // the last axis owns at startup
    assert_eq!(selector.read(&[0.51, -0.5]), Some((1, -0.5))); // noise
    assert_eq!(selector.read(&[0.9, -0.5]), Some((0, 0.9))); // first moved alone
    assert_eq!(selector.read(&[0.0, 0.0]), Some((0, 0.0))); // both moved: ownership kept
    assert_eq!(selector.read(&[0.0, 0.7]), Some((1, 0.7)));
}

#[test]
fn real_resolution_and_independent_axes_without_deadzone() {
    let mut cfg = research();
    cfg.axes = linear();
    for (index, maximum) in [(0usize, 16383u32), (1, 16383), (5, 255)] {
        for value in [0, 1, maximum / 2, maximum - 1, maximum] {
            let s = match index {
                0 => stick(value as u16, 8192, 128, 128, 8, &[]),
                1 => stick(8192, value as u16, 128, 128, 8, &[]),
                _ => stick(8192, 8192, value as u8, 128, 8, &[]),
            };
            let (axes, buttons, hat) = fields(&convert(&mut Mapper::new(&cfg, &devices()), &s, &quadrant(32768, 32768, &[])));
            assert_eq!(u32::from(axes[index]), value * 65535 / maximum);
            assert_eq!((buttons, hat), (0, 8));
        }
    }
    let (axes, _, _) = fields(&convert(&mut mapper(), &neutral_stick(), &quadrant(32768, 32768, &[])));
    assert_eq!((axes[0], axes[1], axes[5]), (32768, 32768, 32768));
    // Out-of-range raw values clamp instead of failing a live frame.
    let (axes, _, _) = fields(&convert(&mut Mapper::new(&cfg, &devices()), &stick(0xffff, 8192, 128, 128, 8, &[]), &quadrant(32768, 32768, &[])));
    assert_eq!(axes[0], 65535);
}

#[test]
fn inversion_on_every_axis_with_and_without_deadzone() {
    let mut cfg = research_with(serde_json::json!({"invert": {"roll": true, "pitch": true, "yaw": true, "throttle": true}}));
    let (axes, _, _) = fields(&convert(&mut Mapper::new(&cfg, &devices()), &stick(0, 16383, 0, 128, 8, &[]), &quadrant(0, 0, &[])));
    assert_eq!((axes[0], axes[1], axes[2], axes[5]), (65535, 0, 65535, 65535));
    cfg.axes = linear();
    let (axes, _, _) = fields(&convert(&mut Mapper::new(&cfg, &devices()), &stick(0, 16383, 255, 128, 8, &[]), &quadrant(0, 0, &[])));
    assert_eq!((axes[0], axes[1], axes[5]), (65535, 0, 0));
}

#[test]
fn right_lever_slider_takeover_and_high_g() {
    let cfg = research_with(serde_json::json!({"throttle_lever": "right"}));
    let mut mapper = Mapper::new(&cfg, &devices());
    assert_eq!(fields(&convert(&mut mapper, &stick(8192, 8192, 128, 0, 8, &[]), &quadrant(0, 12345, &[]))).0[2], 12345);
    let (axes, buttons, _) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 255, 8, &[5]), &quadrant(0, 12345, &[])));
    assert_eq!((axes[2], buttons), (65535, 1 << 4));
    assert_eq!(fields(&convert(&mut mapper, &stick(8192, 8192, 128, 255, 8, &[]), &quadrant(65535, 54321, &[]))).0[2], 54321);
}

#[test]
fn camera_hat_strength_and_neutral() {
    let mut cfg = research();
    cfg.camera_strength = 0.5;
    let mut mapper = Mapper::new(&cfg, &devices());
    let (axes, _, _) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 2, &[]), &quadrant(32768, 32768, &[])));
    assert_eq!((axes[3], axes[4]), (unit16(0.5), 32768));
    let (axes, _, _) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 15, &[]), &quadrant(32768, 32768, &[])));
    assert_eq!((axes[3], axes[4], axes[6]), (32768, 32768, 32768));
}

#[test]
fn buttons_multi_targets_quadrant_bits_and_pov_combinations() {
    let mut cfg = research();
    profile(&mut cfg).buttons.insert("tca-quadrant/b31".into(), vec![14, 15]);
    profile(&mut cfg).buttons.insert("tca-quadrant/b17".into(), vec![13]);
    profile(&mut cfg).pov.insert("tca-quadrant/b3".into(), Direction::Right);
    let mut mapper = Mapper::new(&cfg, &devices());
    let (_, buttons, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 8, &[1, 9, 11]), &quadrant(32768, 32768, &[2, 17, 31, 3])));
    assert_eq!(buttons, 1 | 1 << 6 | 1 << 12 | 1 << 13 | 1 << 14);
    assert_eq!(hat, 1); // up + right
    let (_, _, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 8, &[11, 12]), &quadrant(32768, 32768, &[])));
    assert_eq!(hat, 8); // up + down cancel
    let (_, _, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 8, &[13]), &quadrant(32768, 32768, &[3])));
    assert_eq!(hat, 8); // left + right cancel
    let mut cfg = research();
    profile(&mut cfg).pov.insert("tca-stick/b15".into(), Direction::Up);
    let (_, buttons, hat) = fields(&convert(&mut Mapper::new(&cfg, &devices()), &stick(8192, 8192, 128, 128, 8, &[11, 15, 17]), &quadrant(32768, 32768, &[])));
    assert_eq!((buttons, hat), (0, 0)); // clamped double up; tca-stick/b17 unbound
}

#[test]
fn missing_or_malformed_input_releases_and_resets_throttle() {
    let mut mapper = mapper();
    let good = quadrant(32768, 32768, &[]);
    assert_eq!(convert(&mut mapper, &[], &good), neo::neutral());
    assert_eq!(mapper.convert(&[hid::tca_stick(&neutral_stick()), None]), neo::neutral());
    assert_eq!(convert(&mut mapper, &neutral_stick()[..10], &good), neo::neutral());
    let mut wrong_id = neutral_stick();
    wrong_id[0] = 2;
    assert_eq!(convert(&mut mapper, &wrong_id, &good), neo::neutral());
    assert_eq!(mapper.convert(&tca(&neutral_stick(), &good)[..1]), neo::neutral()); // fewer inputs than devices
    assert_eq!(fields(&convert(&mut mapper, &stick(8192, 8192, 128, 0, 8, &[]), &quadrant(1000, 0, &[]))).0[2], 1000);
    assert_eq!(Mapper::new(&research(), &[]).convert(&[]), neo::neutral());
}

#[test]
fn hat_mode_camera_ignores_hat_mappings_and_mapped_mode_uses_them() {
    let mut cfg = research();
    let p = profile(&mut cfg);
    p.buttons.insert("tca-stick/hat-right".into(), vec![10]);
    p.pov.insert("tca-stick/hat-up".into(), Direction::Up);
    p.pov.insert("tca-stick/hat-down".into(), Direction::Down);
    p.buttons.insert("tca-stick/hat-left".into(), vec![11]);
    let quad = quadrant(32768, 32768, &[]);
    // Camera (default): hat up-right moves the camera, mappings stay inactive.
    let (axes, buttons, hat) = fields(&convert(&mut Mapper::new(&cfg, &devices()), &stick(8192, 8192, 128, 128, 1, &[]), &quad));
    assert_eq!((axes[3], axes[4], buttons, hat), (65535, 0, 0, 8));
    profile(&mut cfg).hat = config::HatMode::Mapped;
    let mut mapper = Mapper::new(&cfg, &devices());
    // Mapped: up-right presses both directions; the camera rests.
    let (axes, buttons, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 1, &[]), &quad));
    assert_eq!((axes[3], axes[4], buttons, hat), (32768, 32768, 1 << 9, 0));
    let (_, buttons, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 6, &[]), &quad));
    assert_eq!((buttons, hat), (1 << 10, 8));
    let (_, buttons, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 4, &[]), &quad));
    assert_eq!((buttons, hat), (0, 4));
    let (_, buttons, hat) = fields(&convert(&mut mapper, &stick(8192, 8192, 128, 128, 8, &[]), &quad));
    assert_eq!((buttons, hat), (0, 8));
}

#[test]
fn tca_decoders_read_report_01_only() {
    let state = hid::tca_stick(&stick(1, 2, 3, 4, 0xf4, &[1, 17])).unwrap();
    assert_eq!(state, InputState { buttons: 1 | 1 << 16, axes: vec![1, 2, 3, 4], hat: Some(4) }); // high nibble ignored
    assert_eq!(hid::tca_stick(&stick(0, 0, 0, 0, 8, &[])).unwrap().hat, None);
    let state = hid::tca_quadrant(&quadrant(5, 6, &[1, 16, 17, 20, 21, 31])).unwrap();
    assert_eq!(state.buttons, 1 | 1 << 15 | 1 << 16 | 1 << 19 | 1 << 20 | 1 << 30);
    assert_eq!((state.axes, state.hat), (vec![5, 6], None));
    assert_eq!(hid::tca_quadrant(&quadrant(0, 0, &[])[..43]), None);
}

#[test]
fn pressed_lists_profile_keys_with_hat_components() {
    let inputs = tca(&stick(0, 0, 0, 0, 1, &[1, 17]), &quadrant(0, 0, &[1, 16, 17, 20, 21, 31]));
    assert_eq!(
        neo::pressed(&devices(), &inputs),
        ["tca-stick/b1", "tca-stick/b17", "tca-stick/hat-up", "tca-stick/hat-right", "tca-quadrant/b1", "tca-quadrant/b16", "tca-quadrant/b17", "tca-quadrant/b20", "tca-quadrant/b21", "tca-quadrant/b31"]
    );
    assert!(neo::pressed(&devices(), &tca(&neutral_stick(), &quadrant(0, 0, &[]))).is_empty());
    assert!(neo::pressed(&devices(), &[None, None]).is_empty());
    assert_eq!(neo::HAT_NAMES.len(), 8);
}

#[test]
fn live_axes_are_scaled_per_device() {
    let axes = neo::axes(&devices(), &[hid::tca_stick(&stick(0, 16383, 255, 0, 8, &[])), None]);
    assert_eq!(axes.len(), 1);
    assert_eq!(axes["tca-stick"], [-1.0, 1.0, 1.0, -1.0]);
}

fn device(id: &str, buttons: u16, axes: &[(&str, u32)], hat: bool) -> DeviceInfo {
    let axes = axes.iter().map(|(name, max)| Axis { name: name.to_string(), max: *max }).collect();
    DeviceInfo { id: id.into(), name: id.into(), vid: 0x044f, pid: 1, layout: Layout { buttons, axes, hat } }
}

#[test]
fn generic_setups_bind_by_name_and_ignore_what_is_absent() {
    let stick = device("t16000m", 16, &[("x", 1023), ("y", 1023), ("rz", 255), ("slider", 255)], true);
    let throttle = device("twcs", 14, &[("z", 1023), ("rz", 1023)], true);
    let devices = [stick.clone(), throttle.clone()];
    let default = config::default_profile(&devices);
    assert_eq!(default.bindings.roll, Some(Binding::new("t16000m", "x", false)));
    assert_eq!(default.bindings.yaw, Some(Binding::new("t16000m", "rz", false)));
    assert_eq!(default.bindings.throttle, [Binding::new("t16000m", "slider", false), Binding::new("twcs", "z", false)]);
    assert_eq!((default.camera_hat.as_deref(), default.buttons.len(), default.split_high_g.enabled), (Some("t16000m"), 15, false));
    let mut cfg = config::defaults();
    cfg.safe_start = false;
    let mut p = default.clone();
    p.camera_hat = Some("twcs".into()); // the throttle's hat drives the camera; the stick's hat is a source
    p.buttons.insert("t16000m/hat-up".into(), vec![12]);
    p.buttons.insert("t16000m/b99".into(), vec![13]); // beyond the stick's buttons: ignored
    p.pov.insert("t16000m/b99".into(), Direction::Up);
    p.buttons.insert("absent/b1".into(), vec![14]);
    p.pov.insert("twcs/hat-down".into(), Direction::Down); // camera hat: inactive
    p.bindings.pitch = Some(Binding::new("t16000m", "missing", false));
    cfg.setups.insert(models::setup_key(&devices), p);
    let mut mapper = Mapper::new(&cfg, &devices);
    let state = |buttons: u128, axes: Vec<u32>, hat| Some(InputState { buttons, axes, hat });
    let report = mapper.convert(&[state(1, vec![0, 512, 128, 255], Some(0)), state(0, vec![1023, 0], Some(4))]);
    let (axes, buttons, hat) = fields(&report);
    assert_eq!((axes[0], axes[1], axes[2], axes[4]), (0, 32768, 65535, 65535)); // pitch unbound; camera down
    assert_eq!((buttons, hat), (1 | 1 << 11, 8));
    // Neither binding: a setup without axes rests.
    let bare = [device("pad", 4, &[], false)];
    assert_eq!(fields(&Mapper::new(&config::defaults(), &bare).convert(&[state(1, vec![], None)])), ([32768; 7], 0, 8));
}

#[test]
fn convention_defaults_cover_odd_controller_sets() {
    // No stick: yaw from a rudder, throttle from a Simulation throttle, the first hat drives the camera.
    let pedals = device("pedals", 0, &[("rudder", 255)], false);
    let throttle = device("box", 8, &[("throttle", 255), ("z", 255)], true);
    let p = config::default_profile(&[pedals, throttle]);
    assert_eq!((p.bindings.roll, p.bindings.yaw), (None, Some(Binding::new("pedals", "rudder", false))));
    assert_eq!(p.bindings.throttle, [Binding::new("box", "throttle", false)]);
    assert_eq!((p.camera_hat.as_deref(), p.buttons.is_empty()), (Some("box"), true));
    // A stick whose Z is not its yaw: Z feeds the throttle; no hat anywhere.
    let stick = device("stick", 20, &[("x", 1), ("y", 1), ("z", 1)], false);
    let p = config::default_profile(&[stick]);
    assert_eq!((p.bindings.yaw, p.bindings.throttle.len(), p.camera_hat), (None, 1, None));
    assert_eq!(p.buttons.len(), 15);
    // No throttle-like axis anywhere; a stick without a hat leaves the camera to another device.
    let stick = device("stick", 2, &[("x", 1), ("y", 1), ("rz", 1)], false);
    let other = device("other", 0, &[], true);
    let p = config::default_profile(&[stick, other]);
    assert_eq!((p.bindings.throttle.len(), p.camera_hat.as_deref()), (0, Some("other")));
    // No stick at all: yaw from any device's Rz.
    let twist = device("twist", 0, &[("rz", 255)], false);
    assert_eq!(config::default_profile(&[twist]).bindings.yaw, Some(Binding::new("twist", "rz", false)));
}

#[test]
fn validation_pattern_matches_pi_stream() {
    let (axes, buttons, hat) = fields(&validation::report(1.0));
    assert_eq!((axes[0], axes[1], buttons, hat), (32767, 32768, 2, 1));
    let (axes, _, _) = fields(&validation::report(7.0));
    assert_eq!(axes[1], 32767);
    let (_, buttons, hat) = fields(&validation::report(15.5));
    assert_eq!((buttons, hat), (0x13, 6));
    assert_eq!(fields(&validation::report(28.0)).0[0], 0);
}

#[test]
fn protocol_frames_round_trip_and_reject_damage() {
    let report = input_report([7; 7], 3, 2);
    let frame = protocol::frame(&report);
    assert_eq!(&frame[..4], b"NE\x01\x01");
    assert_eq!(protocol::parse(&frame), Some(report));
    assert_eq!(protocol::parse(&frame[..10]), None);
    let mut bad = frame;
    bad[2] = 9;
    assert_eq!(protocol::parse(&bad), None);
    let mut bad = frame;
    bad[4] = 2;
    assert_eq!(protocol::parse(&bad), None);
}

#[test]
fn axis_shape_matches_the_shared_vectors() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/axis-shape-vectors.json");
    let vectors: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for case in vectors.as_array().unwrap() {
        let tuning: config::AxisTuning = serde_json::from_value(case["tuning"].clone()).unwrap();
        for point in case["points"].as_array().unwrap() {
            let (input, expected) = (point[0].as_f64().unwrap(), point[1].as_f64().unwrap());
            assert!((neo::shape(input, &tuning) - expected).abs() < 1e-12, "{} at {input}", case["name"]);
        }
    }
}

#[test]
fn detent_bands_brake_cruise_and_boost() {
    let bands = config::Throttle::default(); // idle 10%, cruise 50% ± 3%, boost from 90%
    for (travel, expected) in [(0.0, -1.0), (0.1, -1.0), (0.285, -0.5), (0.47, 0.0), (0.5, 0.0), (0.53, 0.0), (0.9, 1.0), (1.0, 1.0)] {
        assert!((neo::detent(travel, &bands) - expected).abs() < 1e-12, "{travel}");
    }
    assert!((neo::detent(0.71, &bands) - 0.18 / 0.37).abs() < 1e-12);
    let mut cfg = research();
    cfg.throttle.mode = ThrottleMode::Detents;
    profile(&mut cfg).split_high_g.enabled = false;
    let mut mapper = Mapper::new(&cfg, &devices());
    let z = |mapper: &mut Mapper, slider: u8, left: u16, right: u16| fields(&mapper.convert(&tca(&stick(8192, 8192, 128, slider, 8, &[]), &quadrant(left, right, &[])))).0[2];
    // The left lever: idle brakes fully, the cruise detent sits at centre, fully forward boosts fully (NEO Z low).
    assert_eq!([z(&mut mapper, 128, raw(0.0), raw(0.0)), z(&mut mapper, 128, raw(0.5), raw(0.0)), z(&mut mapper, 128, raw(1.0), raw(0.0))], [65535, 32768, 0]);
    // The sidestick slider stays linear when it takes over.
    assert_eq!(z(&mut mapper, 0, raw(1.0), raw(0.0)), 0);
    assert_eq!(z(&mut mapper, 255, raw(1.0), raw(0.0)), 65535);
    // Split High-G follows the forward lever through the same bands.
    profile(&mut cfg).split_high_g.enabled = true;
    let mut split = Mapper::new(&cfg, &devices());
    let (axes, buttons, _) = fields(&split.convert(&tca(&neutral_stick(), &quadrant(raw(0.5), raw(0.0), &[]))));
    assert_eq!((axes[2], buttons), (32768, 0));
    let (axes, buttons, _) = fields(&split.convert(&tca(&neutral_stick(), &quadrant(raw(1.0), raw(0.0), &[]))));
    assert_eq!((axes[2], buttons), (0, 1 << 4));
}

#[test]
fn safe_start_holds_the_throttle_at_centre_until_the_lever_reaches_or_crosses_it() {
    let mut cfg = research();
    cfg.safe_start = true;
    let z = |mapper: &mut Mapper, left: u16| fields(&mapper.convert(&tca(&neutral_stick(), &quadrant(left, 32768, &[])))).0[2];
    let armed = |mapper: &Mapper| mapper.monitor().unwrap().armed;
    // Lever left fully forward: nothing until it comes back to centre, then it follows.
    let mut mapper = Mapper::new(&cfg, &devices());
    assert_eq!((z(&mut mapper, 0), armed(&mapper)), (32768, false));
    assert_eq!(z(&mut mapper, 1000), 32768);
    assert_eq!((z(&mut mapper, 32768), armed(&mapper)), (32768, true));
    assert_eq!(z(&mut mapper, 0), 0);
    // A fast move across centre arms too.
    let mut mapper = Mapper::new(&cfg, &devices());
    assert_eq!(z(&mut mapper, 65535), 32768);
    assert_eq!(z(&mut mapper, 0), 0);
    // New settings mid-flight keep it armed; a lost frame disarms it.
    mapper.reconfigure(&cfg);
    assert_eq!(z(&mut mapper, 0), 0);
    assert_eq!(mapper.convert(&[]), neo::neutral());
    assert_eq!(z(&mut mapper, 0), 32768);
    // Turning safe start off arms at once.
    let mut off = cfg.clone();
    off.safe_start = false;
    mapper.reconfigure(&off);
    assert_eq!(z(&mut mapper, 0), 0);
}

#[test]
fn precision_scales_the_flight_axes_while_its_source_is_held() {
    let mut cfg = research();
    cfg.axes = linear();
    profile(&mut cfg).precision = config::Precision { source: Some("tca-stick/b3".into()), scale: 0.5 };
    let mut mapper = Mapper::new(&cfg, &devices());
    let (axes, _, _) = fields(&mapper.convert(&tca(&stick(16383, 0, 255, 128, 8, &[]), &quadrant(32768, 32768, &[]))));
    assert_eq!((axes[0], axes[1], axes[5], mapper.monitor().unwrap().precision), (65535, 0, 65535, false));
    let (axes, buttons, _) = fields(&mapper.convert(&tca(&stick(16383, 0, 255, 128, 8, &[3]), &quadrant(32768, 32768, &[]))));
    assert_eq!((axes[0], axes[1], axes[5], buttons), (unit16(0.5), unit16(-0.5), unit16(0.5), 1 << 2));
    assert!(mapper.monitor().unwrap().precision);
}

#[test]
fn smoothing_keeps_a_share_of_the_previous_output() {
    let mut cfg = research();
    cfg.axes = linear();
    cfg.axes.roll.smoothing = 0.5;
    let mut mapper = Mapper::new(&cfg, &devices());
    let roll = |mapper: &mut Mapper, x: u16| fields(&mapper.convert(&tca(&stick(x, 8192, 128, 128, 8, &[]), &quadrant(32768, 32768, &[])))).0[0];
    assert_eq!(roll(&mut mapper, 16383), 65535);
    let centre = 2.0 * 8192.0 / 16383.0 - 1.0;
    assert_eq!(roll(&mut mapper, 8192), unit16(1.0 * 0.5 + centre * (1.0 - 0.5)));
}

#[test]
fn behaviours_hold_pulse_pulse_on_change_and_toggle() {
    let mut cfg = research();
    profile(&mut cfg).behaviours = [("tca-stick/b1", Mode::Pulse), ("tca-stick/b2", Mode::PulseChange), ("tca-stick/b3", Mode::Toggle), ("tca-stick/b4", Mode::Hold), ("tca-stick/b11", Mode::Pulse)].iter().map(|(k, m)| (k.to_string(), *m)).collect();
    let quad = quadrant(32768, 32768, &[]);
    let mut mapper = Mapper::new(&cfg, &devices());
    let mut at = |time: u64, pressed: &[u32]| {
        let report = mapper.at(ms(time)).convert(&tca(&stick(8192, 8192, 128, 128, 8, pressed), &quad));
        let (_, buttons, hat) = fields(&report);
        (buttons, hat)
    };
    assert_eq!(at(0, &[]), (0, 8));
    assert_eq!(at(10, &[1, 2, 3, 4, 11]), (0b1111, 0)); // pulses start, toggle on, hold, POV pulse up
    assert_eq!(at(89, &[1, 2, 3, 4, 11]), (0b1111, 0));
    assert_eq!(at(90, &[1, 2, 3, 4, 11]), (0b1100, 8)); // pulses ended after 80 ms
    assert_eq!(at(110, &[]), (0b0110, 8)); // release: pulse on change fires again; toggle stays on
    assert_eq!(at(200, &[]), (0b0100, 8));
    assert_eq!(at(210, &[3]), (0, 8)); // toggle off
    // A source already down on the first frame (a latched switch) is not an edge, and new settings keep that.
    let mut mapper = Mapper::new(&cfg, &devices());
    let down = stick(8192, 8192, 128, 128, 8, &[1, 3]);
    assert_eq!(fields(&mapper.at(ms(0)).convert(&tca(&down, &quad))).1, 0);
    mapper.reconfigure(&cfg);
    assert_eq!(fields(&mapper.at(ms(10)).convert(&tca(&down, &quad))).1, 0);
    // After a lost frame the next frame primes again.
    mapper.convert(&[]);
    assert_eq!(fields(&mapper.at(ms(20)).convert(&tca(&down, &quad))).1, 0);
}

#[test]
fn pause_combo_raises_one_event_per_press() {
    let mut cfg = research();
    profile(&mut cfg).combos.pause = vec!["tca-stick/b5".into(), "tca-quadrant/b6".into()];
    let mut mapper = Mapper::new(&cfg, &devices());
    let mut press = |stick_buttons: &[u32], quadrant_buttons: &[u32]| {
        mapper.convert(&tca(&stick(8192, 8192, 128, 128, 8, stick_buttons), &quadrant(32768, 32768, quadrant_buttons)));
        mapper.take_events()
    };
    assert!(press(&[], &[]).is_empty());
    assert!(press(&[5], &[]).is_empty());
    assert_eq!(press(&[5], &[6]), [Event::TogglePause]);
    assert!(press(&[5], &[6]).is_empty()); // still held
    assert!(press(&[], &[6]).is_empty());
    assert_eq!(press(&[5], &[6]), [Event::TogglePause]);
    // Held when emulation starts: not a press. No combo: never.
    let mut mapper = Mapper::new(&cfg, &devices());
    mapper.convert(&tca(&stick(8192, 8192, 128, 128, 8, &[5]), &quadrant(32768, 32768, &[6])));
    assert!(mapper.take_events().is_empty());
    let mut mapper = Mapper::new(&research(), &devices());
    mapper.convert(&tca(&neutral_stick(), &quadrant(32768, 32768, &[])));
    assert!(mapper.take_events().is_empty());
}

#[test]
fn monitor_reports_raw_inputs_next_to_the_neo_axes() {
    let mut mapper = mapper();
    assert_eq!(mapper.monitor(), None);
    let report = mapper.convert(&tca(&stick(16383, 0, 255, 0, 8, &[]), &quadrant(raw(0.0), raw(1.0), &[])));
    let monitor = mapper.monitor().unwrap();
    assert_eq!((monitor.roll, monitor.pitch, monitor.yaw, monitor.throttle, monitor.levers), (1.0, -1.0, 1.0, Some(1.0), Some([0.0, 1.0]))); // split: the forward lever
    assert_eq!((monitor.axes, monitor.armed, monitor.precision), (fields(&report).0, true, false));
    // Out-of-range raw values are clamped for display too.
    mapper.convert(&tca(&stick(0xffff, 8192, 128, 128, 8, &[]), &quadrant(32768, 32768, &[])));
    assert_eq!(mapper.monitor().unwrap().roll, 1.0);
    mapper.convert(&[]);
    assert_eq!(mapper.monitor(), None);
}

#[test]
fn throttle_selector_reports_its_owner() {
    let mut selector = ThrottleSelector::default();
    assert_eq!(selector.read(&[0.0, 0.0]), Some((1, 0.0)));
    assert_eq!(selector.read(&[0.5, 0.0]), Some((0, 0.5)));
    assert_eq!(selector.read(&[]), None);
}

fn gesture(buttons: &[u8], pov: Option<Direction>) -> config::Target {
    config::Target { buttons: buttons.to_vec(), pov }
}

/// Buttons and POV at `time` with the given sidestick buttons down.
fn frame(mapper: &mut Mapper, time: u64, pressed: &[u32]) -> (u32, u8) {
    let report = mapper.at(ms(time)).convert(&tca(&stick(8192, 8192, 128, 128, 8, pressed), &quadrant(32768, 32768, &[])));
    let (_, buttons, hat) = fields(&report);
    (buttons, hat)
}

#[test]
fn repeat_sends_short_presses_while_held() {
    let mut cfg = research();
    profile(&mut cfg).behaviours.insert("tca-stick/b1".into(), Mode::Repeat); // pulse 80 ms every 120 ms
    let mut mapper = Mapper::new(&cfg, &devices());
    let timeline: Vec<_> = [(0, &[][..]), (10, &[1]), (89, &[1]), (90, &[1]), (130, &[1]), (209, &[1]), (210, &[1]), (250, &[1]), (400, &[])]
        .iter()
        .map(|&(time, pressed)| frame(&mut mapper, time, pressed).0)
        .collect();
    assert_eq!(timeline, [0, 1, 1, 0, 1, 1, 0, 1, 0]);
}

#[test]
fn long_press_holds_its_targets_and_a_tap_becomes_a_pulse_on_release() {
    let mut cfg = research();
    profile(&mut cfg).long_press.insert("tca-stick/b2".into(), gesture(&[3], Some(Direction::Left))); // s2 normally sends NEO 2
    let mut mapper = Mapper::new(&cfg, &devices());
    assert_eq!(frame(&mut mapper, 0, &[]), (0, 8));
    // Tap: nothing while down, a pulse of the normal mapping on release.
    assert_eq!(frame(&mut mapper, 10, &[2]), (0, 8));
    assert_eq!(frame(&mut mapper, 100, &[]), (0b10, 8));
    assert_eq!(frame(&mut mapper, 180, &[]), (0, 8));
    // Hold past 500 ms: the long-press targets are held until release, and the normal mapping never fires.
    assert_eq!(frame(&mut mapper, 200, &[2]), (0, 8));
    assert_eq!(frame(&mut mapper, 699, &[2]), (0, 8));
    assert_eq!(frame(&mut mapper, 700, &[2]), (0b100, 6));
    assert_eq!(frame(&mut mapper, 900, &[2]), (0b100, 6));
    assert_eq!(frame(&mut mapper, 910, &[]), (0, 8));
    assert_eq!(frame(&mut mapper, 1000, &[]), (0, 8));
}

#[test]
fn double_tap_sends_its_targets_and_a_single_tap_waits_out_the_window() {
    let mut cfg = research();
    profile(&mut cfg).double_tap.insert("tca-stick/b2".into(), gesture(&[4], None));
    profile(&mut cfg).long_press.insert("tca-stick/b3".into(), gesture(&[5], None));
    profile(&mut cfg).double_tap.insert("tca-stick/b3".into(), gesture(&[6], None));
    let mut mapper = Mapper::new(&cfg, &devices());
    frame(&mut mapper, 0, &[]);
    // Double tap within 250 ms: one pulse of the double-tap targets, nothing else.
    assert_eq!(frame(&mut mapper, 10, &[2]).0, 0);
    assert_eq!(frame(&mut mapper, 60, &[]).0, 0);
    assert_eq!(frame(&mut mapper, 200, &[2]).0, 0b1000);
    assert_eq!(frame(&mut mapper, 250, &[]).0, 0b1000);
    assert_eq!(frame(&mut mapper, 280, &[]).0, 0);
    assert_eq!(frame(&mut mapper, 600, &[]).0, 0);
    // Single tap: the normal mapping pulses once the window has passed.
    assert_eq!(frame(&mut mapper, 1000, &[2]).0, 0);
    assert_eq!(frame(&mut mapper, 1050, &[]).0, 0);
    assert_eq!(frame(&mut mapper, 1299, &[]).0, 0);
    assert_eq!(frame(&mut mapper, 1300, &[]).0, 0b10);
    assert_eq!(frame(&mut mapper, 1380, &[]).0, 0);
    // With both set, a held second tap stays a double tap: it never becomes a long press.
    assert_eq!(frame(&mut mapper, 2000, &[3]).0, 0);
    assert_eq!(frame(&mut mapper, 2050, &[]).0, 0);
    assert_eq!(frame(&mut mapper, 2100, &[3]).0, 0b10_0000);
    assert_eq!(frame(&mut mapper, 2800, &[3]).0, 0);
    assert_eq!(frame(&mut mapper, 2810, &[]).0, 0);
    // And a single long hold is a long press.
    assert_eq!(frame(&mut mapper, 4000, &[3]).0, 0);
    assert_eq!(frame(&mut mapper, 4500, &[3]).0, 0b1_0000);
    assert_eq!(frame(&mut mapper, 4600, &[]).0, 0);
    assert_eq!(frame(&mut mapper, 5000, &[]).0, 0);
}

#[test]
fn shift_layer_replaces_mappings_while_its_control_is_held() {
    let mut cfg = research();
    // s16 keeps its own mapping (NEO 4) in the profile, but sends nothing while it is the shift control.
    profile(&mut cfg).shift = Some("tca-stick/b16".into());
    profile(&mut cfg).shifted.buttons.insert("tca-stick/b1".into(), vec![9]);
    profile(&mut cfg).shifted.pov.insert("tca-stick/b2".into(), Direction::Down); // pov only: sends no buttons in the shift layer
    let mut mapper = Mapper::new(&cfg, &devices());
    assert_eq!(frame(&mut mapper, 0, &[1, 2, 3]), (0b111, 8));
    assert_eq!(frame(&mut mapper, 10, &[16]), (0, 8)); // the shift control sends nothing itself
    assert_eq!(frame(&mut mapper, 20, &[16, 1]), (1 << 8, 8));
    assert_eq!(frame(&mut mapper, 30, &[16, 2]), (0, 4));
    assert_eq!(frame(&mut mapper, 40, &[16, 3]), (0b100, 8)); // no shifted mapping: the normal one
}

#[test]
fn camera_ramps_in_and_goes_full_after_a_long_press() {
    let mut cfg = research();
    cfg.camera_strength = 0.5;
    cfg.camera.ramp_ms = 200;
    let rx = |mapper: &mut Mapper, time: u64, hat: u8| fields(&mapper.at(ms(time)).convert(&tca(&stick(8192, 8192, 128, 128, hat, &[]), &quadrant(32768, 32768, &[])))).0[3];
    let mut mapper = Mapper::new(&cfg, &devices());
    assert_eq!([rx(&mut mapper, 0, 2), rx(&mut mapper, 100, 2), rx(&mut mapper, 300, 2), rx(&mut mapper, 700, 2)], [32768, unit16(0.25), unit16(0.5), unit16(0.5)]);
    assert_eq!(rx(&mut mapper, 710, 8), 32768); // centred: the ramp starts again
    assert_eq!(rx(&mut mapper, 720, 2), 32768);
    cfg.camera.full_after_hold = true;
    mapper.reconfigure(&cfg);
    assert_eq!([rx(&mut mapper, 1000, 2), rx(&mut mapper, 1220, 2)], [unit16(0.5), 65535]);
    assert_eq!(rx(&mut mapper, 500, 2), 32768); // a clock that goes backwards restarts the ramp, never panics
}

#[test]
fn spare_lever_pans_the_camera_unless_the_hat_is_pressed() {
    let mut cfg = research();
    profile(&mut cfg).split_high_g.enabled = false;
    profile(&mut cfg).camera_lever = Some(Binding::new("tca-quadrant", "right", true));
    let camera = |mapper: &mut Mapper, hat: u8, right: u16| {
        let (axes, _, _) = fields(&mapper.convert(&tca(&stick(8192, 8192, 128, 128, hat, &[]), &quadrant(32768, right, &[]))));
        (axes[3], axes[4])
    };
    let mut mapper = Mapper::new(&cfg, &devices()); // throttle on the left lever: the right lever is spare
    assert_eq!(camera(&mut mapper, 8, raw(1.0)), (65535, 32768));
    assert_eq!(camera(&mut mapper, 8, raw(0.0)), (0, 32768));
    assert_eq!(camera(&mut mapper, 8, raw(0.52)), (32768, 32768)); // inside the centre band
    assert_eq!(camera(&mut mapper, 0, raw(1.0)), (32768, 0)); // hat up wins
    profile(&mut cfg).camera_lever = Some(Binding::new("tca-quadrant", "left", true));
    profile(&mut cfg).bindings.throttle[1].axis = "right".into();
    let mut mapper = Mapper::new(&cfg, &devices());
    let (axes, _, _) = fields(&mapper.convert(&tca(&neutral_stick(), &quadrant(raw(1.0), raw(0.5), &[]))));
    assert_eq!(axes[3], 65535);
    // Any axis can be the camera lever, as read (here the sidestick slider, not inverted).
    profile(&mut cfg).camera_lever = Some(Binding::new("tca-stick", "slider", false));
    let mut mapper = Mapper::new(&cfg, &devices());
    let (axes, _, _) = fields(&mapper.convert(&tca(&stick(8192, 8192, 128, 255, 8, &[]), &quadrant(32768, 32768, &[]))));
    assert_eq!(axes[3], 65535);
    assert_eq!(mapper.monitor().unwrap().camera_lever, Some(1.0));
}

#[test]
fn yaw_can_be_off_or_come_from_the_lever_difference() {
    let mut cfg = research();
    profile(&mut cfg).yaw_source = config::YawSource::Off;
    let (axes, _, _) = fields(&Mapper::new(&cfg, &devices()).convert(&tca(&stick(8192, 8192, 255, 128, 8, &[]), &quadrant(32768, 32768, &[]))));
    assert_eq!(axes[5], 32768);
    profile(&mut cfg).split_high_g.enabled = false;
    profile(&mut cfg).yaw_source = config::YawSource::LeverDifferential;
    profile(&mut cfg).bindings.yaw.as_mut().unwrap().invert = true;
    let mut mapper = Mapper::new(&cfg, &devices());
    // Left lever forward, right back: full yaw (inverted here), and the throttle follows the average (centre).
    let (axes, _, _) = fields(&mapper.convert(&tca(&neutral_stick(), &quadrant(raw(1.0), raw(0.0), &[]))));
    assert_eq!((axes[5], axes[2]), (0, 32768));
    cfg.throttle.mode = ThrottleMode::Detents;
    profile(&mut cfg).bindings.yaw.as_mut().unwrap().invert = false;
    let mut mapper = Mapper::new(&cfg, &devices());
    let (axes, _, _) = fields(&mapper.convert(&tca(&neutral_stick(), &quadrant(raw(0.4), raw(0.6), &[]))));
    assert_eq!((axes[2], axes[5]), (32768, unit16(neo::shape(-0.2, &cfg.axes.yaw))));
    // Without both levers reporting, lever yaw rests at centre.
    profile(&mut cfg).split_high_g.levers[1].device = "absent".into();
    let mut mapper = Mapper::new(&cfg, &devices());
    let (axes, _, _) = fields(&mapper.convert(&tca(&neutral_stick(), &quadrant(raw(0.4), raw(0.6), &[]))));
    assert_eq!((axes[5], mapper.monitor().unwrap().yaw), (32768, 0.0));
}

#[test]
fn next_profile_combo_and_monitor_report_what_is_sent() {
    let mut cfg = research();
    profile(&mut cfg).combos.next_profile = vec!["tca-stick/b16".into()];
    profile(&mut cfg).shift = Some("tca-stick/b17".into());
    let mut mapper = Mapper::new(&cfg, &devices());
    mapper.convert(&tca(&neutral_stick(), &quadrant(32768, 32768, &[])));
    mapper.convert(&tca(&stick(8192, 8192, 128, 128, 8, &[16, 17, 1, 11]), &quadrant(32768, 32768, &[])));
    assert_eq!(mapper.take_events(), [Event::NextProfile]);
    let monitor = mapper.monitor().unwrap();
    assert_eq!((monitor.shift, monitor.buttons, monitor.pov), (true, 0b1001, 0)); // S1 → NEO 1, S16 → NEO 4, S11 → POV up
}

#[test]
fn head_tracking_moves_the_camera_unless_the_hat_is_pressed() {
    let mut cfg = research();
    cfg.head_tracking.enabled = true;
    cfg.head_tracking.tuning = config::AxisTuning { deadzone: 0.0, ..config::AxisTuning::default() };
    let camera = |mapper: &mut Mapper, hat: u8, head: Option<[f64; 2]>| {
        let (axes, _, _) = fields(&mapper.head(head).convert(&tca(&stick(8192, 8192, 128, 128, hat, &[]), &quadrant(32768, 32768, &[]))));
        (axes[3], axes[4])
    };
    let mut mapper = Mapper::new(&cfg, &devices());
    // Half the yaw range right, a full look up (camera up is negative Ry); beyond the range is full deflection.
    assert_eq!(camera(&mut mapper, 8, Some([22.5, 30.0])), (unit16(0.5), 0));
    assert_eq!(camera(&mut mapper, 8, Some([-90.0, -60.0])), (0, 65535));
    assert_eq!(mapper.monitor().unwrap().head, Some([-90.0, -60.0]));
    assert_eq!(camera(&mut mapper, 2, Some([-90.0, 0.0])), (65535, 32768)); // the hat (right) wins
    assert_eq!(camera(&mut mapper, 8, None), (32768, 32768)); // no fresh sample: the hat, centred
    cfg.head_tracking.invert_yaw = true;
    cfg.head_tracking.invert_pitch = true;
    mapper.reconfigure(&cfg);
    assert_eq!(camera(&mut mapper, 8, Some([22.5, 30.0])), (unit16(-0.5), 65535));
    cfg.head_tracking.enabled = false;
    mapper.reconfigure(&cfg);
    assert_eq!(camera(&mut mapper, 8, Some([22.5, 30.0])), (32768, 32768));
    assert_eq!(mapper.monitor().unwrap().head, None);
}
