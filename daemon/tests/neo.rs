use neo_daemon::config::{self, Direction, Lever, Mode, ThrottleMode};
use neo_daemon::neo::{self, centered, input_report, unit16, Event, Mapper, ThrottleSelector};
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

fn fields(report: &neo::Report) -> ([u16; 7], u32, u8) {
    let axes = std::array::from_fn(|i| u16::from_le_bytes([report[4 + 2 * i], report[5 + 2 * i]]));
    let packed = u32::from_le_bytes([report[1], report[2], report[3], 0]);
    (axes, packed >> 4, report[1] & 15)
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

/// The research mapping from tca_to_neo.py (DEFAULT_BINDINGS / ORDERS), which produced the Python vectors,
/// with the settings these tests were written against: throttle not inverted, split High-G at 95% / 5%.
fn research() -> config::Config {
    let mut cfg = config::defaults();
    // The research converter held every source and had no safe start.
    cfg.behaviours.clear();
    cfg.safe_start = false;
    cfg.invert = config::Invert::default();
    cfg.split_high_g = config::SplitHighG { enabled: true, forward: 0.95, back: 0.05 };
    cfg.buttons = [("s1", 1), ("s2", 2), ("s3", 3), ("s4", 4), ("s5", 5), ("s6", 7), ("s7", 8), ("s8", 9), ("s9", 1), ("s10", 12), ("s16", 4), ("q1", 2), ("q2", 7)]
        .iter()
        .map(|(k, v)| (k.to_string(), vec![*v]))
        .collect();
    cfg.pov = [("s11", Direction::Up), ("s12", Direction::Down), ("s13", Direction::Left), ("s14", Direction::Right)]
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    cfg
}

fn mapper() -> Mapper {
    Mapper::new(&research())
}

/// Roll, pitch and yaw passed through untouched.
fn linear() -> config::Axes {
    let axis = config::AxisTuning { deadzone: 0.0, ..config::AxisTuning::default() };
    config::Axes { roll: axis, pitch: axis, yaw: axis }
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
        let report = mapper.convert(Some(&stick), Some(&quadrant));
        assert_eq!(report.to_vec(), hex(vector["neo_report"].as_str().unwrap()), "{}", vector["name"]);
    }
}

#[test]
fn default_profile_maps_sidestick_numbers_and_recorded_quadrant_controls() {
    let mut mapper = Mapper::new(&config::defaults());
    let neutral_quadrant = quadrant(32768, 32768, &[]);
    for n in 1..=15u32 {
        let (_, buttons, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, &[n])), Some(&neutral_quadrant)));
        assert_eq!((buttons, hat), (1 << (n - 1), 8), "sidestick {n}");
    }
    // Pulsed sources (ENG switches, mode selector) fire when they switch on after the first frame.
    for (button, neo) in [(1, 5), (2, 5), (3, 6), (4, 6), (5, 9), (6, 6), (7, 7), (8, 8)] {
        let mut mapper = Mapper::new(&config::defaults());
        mapper.convert(Some(&neutral_stick()), Some(&neutral_quadrant));
        let (_, buttons, _) = fields(&mapper.convert(Some(&neutral_stick()), Some(&quadrant(32768, 32768, &[button]))));
        assert_eq!(buttons, 1 << (neo - 1), "quadrant {button}");
    }
    // Tuned settings: no axis inverted (checked in AC8), split High-G at 75% / 25%.
    let defaults = config::defaults();
    assert!(!defaults.invert.throttle && !defaults.invert.roll && !defaults.invert.pitch && !defaults.invert.yaw);
    assert_eq!((defaults.split_high_g.enabled, defaults.split_high_g.forward, defaults.split_high_g.back), (true, 0.75, 0.25));
    // Sidestick 16/17 and the remaining quadrant inputs are unmapped; no control sends the POV; the hat is camera.
    let (axes, buttons, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 0, &[16, 17])), Some(&quadrant(32768, 32768, &[9, 31]))));
    assert_eq!((buttons, hat, axes[4]), (0, 8, 0));
    assert!(config::defaults().pov.is_empty());
}

#[test]
fn split_levers_hold_high_g_and_the_throttle_follows_the_forward_lever() {
    // The TCA reports a lever fully forward as raw 0 and at idle as raw 65535.
    let neutral = neutral_stick();
    let (forward_raw, idle_raw) = (0u16, 65535u16);
    let mut mapper = mapper();
    let normal = |mapper: &mut Mapper, raw: u16| fields(&mapper.convert(Some(&neutral), Some(&quadrant(raw, raw, &[])))).0[2];
    // Left forward, right back: High-G (NEO 5) held, throttle as if the forward lever alone were the throttle.
    let (axes, buttons, _) = fields(&mapper.convert(Some(&neutral), Some(&quadrant(forward_raw, idle_raw, &[]))));
    assert_eq!((axes[2], buttons), (0, 1 << 4));
    // Either order; thresholds are inclusive (95% / 5% of travel).
    let forward = (0.05f64 * 65535.0).floor() as u16;
    let back = (0.95f64 * 65535.0).ceil() as u16;
    let (axes, buttons, _) = fields(&mapper.convert(Some(&neutral), Some(&quadrant(back, forward, &[]))));
    assert_eq!((axes[2], buttons), (normal(&mut mapper, forward), 1 << 4));
    // Just outside either threshold: no High-G.
    assert_eq!(fields(&mapper.convert(Some(&neutral), Some(&quadrant(forward + 2, idle_raw, &[])))).1, 0);
    assert_eq!(fields(&mapper.convert(Some(&neutral), Some(&quadrant(forward_raw, back - 2, &[])))).1, 0);
    // Throttle inverted, thresholds 75% / 25%: the split throttle still follows the forward lever and is inverted
    // exactly like the normal throttle, never the back lever's idle.
    let mut cfg = research();
    cfg.invert.throttle = true;
    cfg.split_high_g.forward = 0.75;
    cfg.split_high_g.back = 0.25;
    let mut inverted = Mapper::new(&cfg);
    let idle = normal(&mut inverted, idle_raw);
    let full_normal = normal(&mut inverted, forward_raw);
    assert_eq!((idle, full_normal), (0, 65535));
    for levers in [(forward_raw, idle_raw), (idle_raw, forward_raw)] {
        let (axes, buttons, _) = fields(&inverted.convert(Some(&neutral), Some(&quadrant(levers.0, levers.1, &[]))));
        assert_eq!((axes[2], buttons), (full_normal, 1 << 4), "{levers:?}");
    }
    let (axes, buttons, _) = fields(&inverted.convert(Some(&neutral), Some(&quadrant(16383, 49152, &[]))));
    assert_eq!((axes[2], buttons), (normal(&mut inverted, 16383), 1 << 4)); // 75% / 25%: inclusive
    // Disabled: levers act normally.
    let mut cfg = research();
    cfg.split_high_g.enabled = false;
    assert_eq!(fields(&Mapper::new(&cfg).convert(Some(&neutral), Some(&quadrant(forward_raw, idle_raw, &[])))).1, 0);
    // Combined with a mapped High-G button the bit is simply set once.
    let (_, buttons, _) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, &[5])), Some(&quadrant(idle_raw, forward_raw, &[]))));
    assert_eq!(buttons, 1 << 4);
}

#[test]
fn lever_positions_and_split_rule() {
    // Travel: raw 65535 is idle (0.0), raw 0 is fully forward (1.0).
    assert_eq!(neo::levers(&quadrant(65535, 0, &[])), [0.0, 1.0]);
    let rule = config::SplitHighG::default();
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
    assert_eq!(centered(0.03, 0.04), 0.0);
    assert_eq!(centered(-2.0, 0.0), -1.0);
    assert!((centered(0.52, 0.04) - 0.5).abs() < 1e-12);
    assert!((centered(-0.52, 0.04) + 0.5).abs() < 1e-12);
    assert_eq!(unit16(0.0), 32768); // 32767.5 rounds to even
    assert_eq!(unit16(-5.0), 0);
    assert_eq!(unit16(5.0), 65535);
}

#[test]
fn throttle_last_meaningful_move_wins() {
    let mut selector = ThrottleSelector::default();
    assert_eq!(selector.read(0.5, -0.5), -0.5); // quadrant owns at startup
    assert_eq!(selector.read(0.51, -0.5), -0.5); // noise
    assert_eq!(selector.read(0.9, -0.5), 0.9); // stick moved alone
    assert_eq!(selector.read(0.0, 0.0), 0.0); // both moved: ownership kept
    assert_eq!(selector.read(0.0, 0.7), 0.7);
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
            let (axes, buttons, hat) = fields(&Mapper::new(&cfg).convert(Some(&s), Some(&quadrant(32768, 32768, &[]))));
            assert_eq!(u32::from(axes[index]), value * 65535 / maximum);
            assert_eq!((buttons, hat), (0, 8));
        }
    }
    let (axes, _, _) = fields(&mapper().convert(Some(&neutral_stick()), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!((axes[0], axes[1], axes[5]), (32768, 32768, 32768));
    // Out-of-range raw values clamp instead of failing a live frame.
    let (axes, _, _) = fields(&Mapper::new(&cfg).convert(Some(&stick(0xffff, 8192, 128, 128, 8, &[])), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!(axes[0], 65535);
}

#[test]
fn inversion_on_every_axis_with_and_without_deadzone() {
    let mut cfg = research();
    cfg.invert = config::Invert { roll: true, pitch: true, yaw: true, throttle: true };
    let (axes, _, _) = fields(&Mapper::new(&cfg).convert(Some(&stick(0, 16383, 0, 128, 8, &[])), Some(&quadrant(0, 0, &[]))));
    assert_eq!((axes[0], axes[1], axes[2], axes[5]), (65535, 0, 65535, 65535));
    cfg.axes = linear();
    let (axes, _, _) = fields(&Mapper::new(&cfg).convert(Some(&stick(0, 16383, 255, 128, 8, &[])), Some(&quadrant(0, 0, &[]))));
    assert_eq!((axes[0], axes[1], axes[5]), (65535, 0, 0));
}

#[test]
fn right_lever_slider_takeover_and_high_g() {
    let mut cfg = research();
    cfg.throttle_lever = Lever::Right;
    let mut mapper = Mapper::new(&cfg);
    assert_eq!(fields(&mapper.convert(Some(&stick(8192, 8192, 128, 0, 8, &[])), Some(&quadrant(0, 12345, &[])))).0[2], 12345);
    let (axes, buttons, _) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 255, 8, &[5])), Some(&quadrant(0, 12345, &[]))));
    assert_eq!((axes[2], buttons), (65535, 1 << 4));
    assert_eq!(fields(&mapper.convert(Some(&stick(8192, 8192, 128, 255, 8, &[])), Some(&quadrant(65535, 54321, &[])))).0[2], 54321);
}

#[test]
fn camera_hat_strength_and_neutral() {
    let mut cfg = research();
    cfg.camera_strength = 0.5;
    let mut mapper = Mapper::new(&cfg);
    let (axes, _, _) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 2, &[])), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!((axes[3], axes[4]), (unit16(0.5), 32768));
    let (axes, _, _) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 15, &[])), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!((axes[3], axes[4], axes[6]), (32768, 32768, 32768));
}

#[test]
fn buttons_multi_targets_quadrant_bits_and_pov_combinations() {
    let mut cfg = research();
    cfg.buttons.insert("q31".into(), vec![14, 15]);
    cfg.buttons.insert("q17".into(), vec![13]);
    cfg.pov.insert("q3".into(), Direction::Right);
    let mut mapper = Mapper::new(&cfg);
    let (_, buttons, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, &[1, 9, 11])), Some(&quadrant(32768, 32768, &[2, 17, 31, 3]))));
    assert_eq!(buttons, 1 | 1 << 6 | 1 << 12 | 1 << 13 | 1 << 14);
    assert_eq!(hat, 1); // up + right
    let (_, _, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, &[11, 12])), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!(hat, 8); // up + down cancel
    let (_, _, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, &[13])), Some(&quadrant(32768, 32768, &[3]))));
    assert_eq!(hat, 8); // left + right cancel
    let mut cfg = research();
    cfg.pov.insert("s15".into(), Direction::Up);
    let (_, buttons, hat) = fields(&Mapper::new(&cfg).convert(Some(&stick(8192, 8192, 128, 128, 8, &[11, 15, 17])), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!((buttons, hat), (0, 0)); // clamped double up; s17 unbound
}

#[test]
fn missing_or_malformed_input_releases_and_resets_throttle() {
    let mut mapper = mapper();
    let good = quadrant(32768, 32768, &[]);
    assert_eq!(mapper.convert(None, Some(&good)), neo::neutral());
    assert_eq!(mapper.convert(Some(&neutral_stick()), None), neo::neutral());
    assert_eq!(mapper.convert(Some(&neutral_stick()[..10]), Some(&good)), neo::neutral());
    let mut wrong_id = neutral_stick();
    wrong_id[0] = 2;
    assert_eq!(mapper.convert(Some(&wrong_id), Some(&good)), neo::neutral());
    assert_eq!(fields(&mapper.convert(Some(&stick(8192, 8192, 128, 0, 8, &[])), Some(&quadrant(1000, 0, &[])))).0[2], 1000);
}

#[test]
fn hat_mode_camera_ignores_hat_mappings_and_mapped_mode_uses_them() {
    let mut cfg = research();
    cfg.buttons.insert("hat-right".into(), vec![10]);
    cfg.pov.insert("hat-up".into(), Direction::Up);
    cfg.pov.insert("hat-down".into(), Direction::Down);
    cfg.buttons.insert("hat-left".into(), vec![11]);
    let quad = quadrant(32768, 32768, &[]);
    // Camera (default): hat up-right moves the camera, mappings stay inactive.
    let (axes, buttons, hat) = fields(&Mapper::new(&cfg).convert(Some(&stick(8192, 8192, 128, 128, 1, &[])), Some(&quad)));
    assert_eq!((axes[3], axes[4], buttons, hat), (65535, 0, 0, 8));
    cfg.hat = config::HatMode::Mapped;
    let mut mapper = Mapper::new(&cfg);
    // Mapped: up-right presses both directions; the camera rests.
    let (axes, buttons, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 1, &[])), Some(&quad)));
    assert_eq!((axes[3], axes[4], buttons, hat), (32768, 32768, 1 << 9, 0));
    let (_, buttons, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 6, &[])), Some(&quad)));
    assert_eq!((buttons, hat), (1 << 10, 8));
    let (_, buttons, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 4, &[])), Some(&quad)));
    assert_eq!((buttons, hat), (0, 4));
    let (_, buttons, hat) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, &[])), Some(&quad)));
    assert_eq!((buttons, hat), (0, 8));
}

#[test]
fn hat_names_every_direction_and_none_when_centred() {
    let names: Vec<_> = (0..8).map(|h| neo::hat(&stick(0, 0, 0, 0, h, &[])).unwrap()).collect();
    assert_eq!(names, neo::HAT_NAMES);
    assert_eq!(neo::hat(&stick(0, 0, 0, 0, 8, &[])), None);
    assert_eq!(neo::hat(&stick(0, 0, 0, 0, 0xf4, &[])), Some("down")); // high nibble ignored
}

#[test]
fn pressed_lists_config_keys() {
    assert_eq!(neo::pressed(&stick(0, 0, 0, 0, 8, &[1, 17]), &quadrant(0, 0, &[1, 16, 17, 20, 21, 31])), ["s1", "s17", "q1", "q16", "q17", "q20", "q21", "q31"]);
    assert!(neo::pressed(&neutral_stick(), &quadrant(0, 0, &[])).is_empty());
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
    cfg.split_high_g.enabled = false;
    let mut mapper = Mapper::new(&cfg);
    let z = |mapper: &mut Mapper, slider: u8, left: u16, right: u16| fields(&mapper.convert(Some(&stick(8192, 8192, 128, slider, 8, &[])), Some(&quadrant(left, right, &[])))).0[2];
    // The left lever: idle brakes fully, the cruise detent sits at centre, fully forward boosts fully (NEO Z low).
    assert_eq!([z(&mut mapper, 128, raw(0.0), raw(0.0)), z(&mut mapper, 128, raw(0.5), raw(0.0)), z(&mut mapper, 128, raw(1.0), raw(0.0))], [65535, 32768, 0]);
    // The sidestick slider stays linear when it takes over.
    assert_eq!(z(&mut mapper, 0, raw(1.0), raw(0.0)), 0);
    assert_eq!(z(&mut mapper, 255, raw(1.0), raw(0.0)), 65535);
    // Split High-G follows the forward lever through the same bands.
    cfg.split_high_g.enabled = true;
    let mut split = Mapper::new(&cfg);
    let (axes, buttons, _) = fields(&split.convert(Some(&neutral_stick()), Some(&quadrant(raw(0.5), raw(0.0), &[]))));
    assert_eq!((axes[2], buttons), (32768, 0));
    let (axes, buttons, _) = fields(&split.convert(Some(&neutral_stick()), Some(&quadrant(raw(1.0), raw(0.0), &[]))));
    assert_eq!((axes[2], buttons), (0, 1 << 4));
}

#[test]
fn safe_start_holds_the_throttle_at_centre_until_the_lever_reaches_or_crosses_it() {
    let mut cfg = research();
    cfg.safe_start = true;
    let z = |mapper: &mut Mapper, left: u16| fields(&mapper.convert(Some(&neutral_stick()), Some(&quadrant(left, 32768, &[])))).0[2];
    let armed = |mapper: &Mapper| mapper.monitor().unwrap().armed;
    // Lever left fully forward: nothing until it comes back to centre, then it follows.
    let mut mapper = Mapper::new(&cfg);
    assert_eq!((z(&mut mapper, 0), armed(&mapper)), (32768, false));
    assert_eq!(z(&mut mapper, 1000), 32768);
    assert_eq!((z(&mut mapper, 32768), armed(&mapper)), (32768, true));
    assert_eq!(z(&mut mapper, 0), 0);
    // A fast move across centre arms too.
    let mut mapper = Mapper::new(&cfg);
    assert_eq!(z(&mut mapper, 65535), 32768);
    assert_eq!(z(&mut mapper, 0), 0);
    // New settings mid-flight keep it armed; a lost frame disarms it.
    mapper.reconfigure(&cfg);
    assert_eq!(z(&mut mapper, 0), 0);
    assert_eq!(mapper.convert(None, None), neo::neutral());
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
    cfg.precision = config::Precision { source: Some("s3".into()), scale: 0.5 };
    let mut mapper = Mapper::new(&cfg);
    let (axes, _, _) = fields(&mapper.convert(Some(&stick(16383, 0, 255, 128, 8, &[])), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!((axes[0], axes[1], axes[5], mapper.monitor().unwrap().precision), (65535, 0, 65535, false));
    let (axes, buttons, _) = fields(&mapper.convert(Some(&stick(16383, 0, 255, 128, 8, &[3])), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!((axes[0], axes[1], axes[5], buttons), (unit16(0.5), unit16(-0.5), unit16(0.5), 1 << 2));
    assert!(mapper.monitor().unwrap().precision);
}

#[test]
fn smoothing_keeps_a_share_of_the_previous_output() {
    let mut cfg = research();
    cfg.axes = linear();
    cfg.axes.roll.smoothing = 0.5;
    let mut mapper = Mapper::new(&cfg);
    let roll = |mapper: &mut Mapper, x: u16| fields(&mapper.convert(Some(&stick(x, 8192, 128, 128, 8, &[])), Some(&quadrant(32768, 32768, &[])))).0[0];
    assert_eq!(roll(&mut mapper, 16383), 65535);
    let centre = 2.0 * 8192.0 / 16383.0 - 1.0;
    assert_eq!(roll(&mut mapper, 8192), unit16(1.0 * 0.5 + centre * (1.0 - 0.5)));
}

#[test]
fn behaviours_hold_pulse_pulse_on_change_and_toggle() {
    let mut cfg = research();
    cfg.behaviours = [("s1", Mode::Pulse), ("s2", Mode::PulseChange), ("s3", Mode::Toggle), ("s4", Mode::Hold), ("s11", Mode::Pulse)].iter().map(|(k, m)| (k.to_string(), *m)).collect();
    let quad = quadrant(32768, 32768, &[]);
    let mut mapper = Mapper::new(&cfg);
    let mut at = |time: u64, pressed: &[u32]| {
        let report = mapper.at(ms(time)).convert(Some(&stick(8192, 8192, 128, 128, 8, pressed)), Some(&quad));
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
    let mut mapper = Mapper::new(&cfg);
    let down = stick(8192, 8192, 128, 128, 8, &[1, 3]);
    assert_eq!(fields(&mapper.at(ms(0)).convert(Some(&down), Some(&quad))).1, 0);
    mapper.reconfigure(&cfg);
    assert_eq!(fields(&mapper.at(ms(10)).convert(Some(&down), Some(&quad))).1, 0);
    // After a lost frame the next frame primes again.
    mapper.convert(None, None);
    assert_eq!(fields(&mapper.at(ms(20)).convert(Some(&down), Some(&quad))).1, 0);
}

#[test]
fn pause_combo_raises_one_event_per_press() {
    let mut cfg = research();
    cfg.combos.pause = vec!["s5".into(), "q6".into()];
    let mut mapper = Mapper::new(&cfg);
    let mut press = |stick_buttons: &[u32], quadrant_buttons: &[u32]| {
        mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, stick_buttons)), Some(&quadrant(32768, 32768, quadrant_buttons)));
        mapper.take_events()
    };
    assert!(press(&[], &[]).is_empty());
    assert!(press(&[5], &[]).is_empty());
    assert_eq!(press(&[5], &[6]), [Event::TogglePause]);
    assert!(press(&[5], &[6]).is_empty()); // still held
    assert!(press(&[], &[6]).is_empty());
    assert_eq!(press(&[5], &[6]), [Event::TogglePause]);
    // Held when emulation starts: not a press. No combo: never.
    let mut mapper = Mapper::new(&cfg);
    mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, &[5])), Some(&quadrant(32768, 32768, &[6])));
    assert!(mapper.take_events().is_empty());
    let mut mapper = Mapper::new(&research());
    mapper.convert(Some(&neutral_stick()), Some(&quadrant(32768, 32768, &[])));
    assert!(mapper.take_events().is_empty());
}

#[test]
fn monitor_reports_raw_inputs_next_to_the_neo_axes() {
    let mut mapper = mapper();
    assert_eq!(mapper.monitor(), None);
    let report = mapper.convert(Some(&stick(16383, 0, 255, 0, 8, &[])), Some(&quadrant(raw(0.0), raw(1.0), &[])));
    let monitor = mapper.monitor().unwrap();
    assert_eq!((monitor.roll, monitor.pitch, monitor.yaw, monitor.slider, monitor.levers), (1.0, -1.0, 1.0, -1.0, [0.0, 1.0]));
    assert_eq!((monitor.axes, monitor.armed, monitor.precision), (fields(&report).0, true, false));
    // Out-of-range raw values are clamped for display too.
    mapper.convert(Some(&stick(0xffff, 8192, 128, 128, 8, &[])), Some(&quadrant(32768, 32768, &[])));
    assert_eq!(mapper.monitor().unwrap().roll, 1.0);
    mapper.convert(None, None);
    assert_eq!(mapper.monitor(), None);
}

#[test]
fn throttle_selector_reports_its_owner() {
    let mut selector = ThrottleSelector::default();
    assert_eq!(selector.active(), 1);
    selector.read(0.0, 0.0);
    selector.read(0.5, 0.0);
    assert_eq!(selector.active(), 0);
}

fn gesture(buttons: &[u8], pov: Option<Direction>) -> config::Target {
    config::Target { buttons: buttons.to_vec(), pov }
}

/// Buttons and POV at `time` with the given sidestick buttons down.
fn frame(mapper: &mut Mapper, time: u64, pressed: &[u32]) -> (u32, u8) {
    let report = mapper.at(ms(time)).convert(Some(&stick(8192, 8192, 128, 128, 8, pressed)), Some(&quadrant(32768, 32768, &[])));
    let (_, buttons, hat) = fields(&report);
    (buttons, hat)
}

#[test]
fn repeat_sends_short_presses_while_held() {
    let mut cfg = research();
    cfg.behaviours.insert("s1".into(), Mode::Repeat); // pulse 80 ms every 120 ms
    let mut mapper = Mapper::new(&cfg);
    let timeline: Vec<_> = [(0, &[][..]), (10, &[1]), (89, &[1]), (90, &[1]), (130, &[1]), (209, &[1]), (210, &[1]), (250, &[1]), (400, &[])]
        .iter()
        .map(|&(time, pressed)| frame(&mut mapper, time, pressed).0)
        .collect();
    assert_eq!(timeline, [0, 1, 1, 0, 1, 1, 0, 1, 0]);
}

#[test]
fn long_press_holds_its_targets_and_a_tap_becomes_a_pulse_on_release() {
    let mut cfg = research();
    cfg.long_press.insert("s2".into(), gesture(&[3], Some(Direction::Left))); // s2 normally sends NEO 2
    let mut mapper = Mapper::new(&cfg);
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
    cfg.double_tap.insert("s2".into(), gesture(&[4], None));
    cfg.long_press.insert("s3".into(), gesture(&[5], None));
    cfg.double_tap.insert("s3".into(), gesture(&[6], None));
    let mut mapper = Mapper::new(&cfg);
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
    cfg.shift = Some("s16".into());
    cfg.shifted.buttons.insert("s1".into(), vec![9]);
    cfg.shifted.pov.insert("s2".into(), Direction::Down); // pov only: sends no buttons in the shift layer
    let mut mapper = Mapper::new(&cfg);
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
    let rx = |mapper: &mut Mapper, time: u64, hat: u8| fields(&mapper.at(ms(time)).convert(Some(&stick(8192, 8192, 128, 128, hat, &[])), Some(&quadrant(32768, 32768, &[])))).0[3];
    let mut mapper = Mapper::new(&cfg);
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
    cfg.split_high_g.enabled = false;
    cfg.camera.lever = true;
    let camera = |mapper: &mut Mapper, hat: u8, right: u16| {
        let (axes, _, _) = fields(&mapper.convert(Some(&stick(8192, 8192, 128, 128, hat, &[])), Some(&quadrant(32768, right, &[]))));
        (axes[3], axes[4])
    };
    let mut mapper = Mapper::new(&cfg); // throttle on the left lever: the right lever is spare
    assert_eq!(camera(&mut mapper, 8, raw(1.0)), (65535, 32768));
    assert_eq!(camera(&mut mapper, 8, raw(0.0)), (0, 32768));
    assert_eq!(camera(&mut mapper, 8, raw(0.52)), (32768, 32768)); // inside the centre band
    assert_eq!(camera(&mut mapper, 0, raw(1.0)), (32768, 0)); // hat up wins
    cfg.throttle_lever = Lever::Right;
    let mut mapper = Mapper::new(&cfg);
    let (axes, _, _) = fields(&mapper.convert(Some(&neutral_stick()), Some(&quadrant(raw(1.0), raw(0.5), &[]))));
    assert_eq!(axes[3], 65535);
}

#[test]
fn yaw_can_be_off_or_come_from_the_lever_difference() {
    let mut cfg = research();
    cfg.yaw_source = config::YawSource::Off;
    let (axes, _, _) = fields(&Mapper::new(&cfg).convert(Some(&stick(8192, 8192, 255, 128, 8, &[])), Some(&quadrant(32768, 32768, &[]))));
    assert_eq!(axes[5], 32768);
    cfg.split_high_g.enabled = false;
    cfg.yaw_source = config::YawSource::LeverDifferential;
    cfg.invert.yaw = true;
    let mut mapper = Mapper::new(&cfg);
    // Left lever forward, right back: full yaw (inverted here), and the throttle follows the average (centre).
    let (axes, _, _) = fields(&mapper.convert(Some(&neutral_stick()), Some(&quadrant(raw(1.0), raw(0.0), &[]))));
    assert_eq!((axes[5], axes[2]), (0, unit16(2.0 * 32767.0 / 65535.0 - 1.0)));
    cfg.throttle.mode = ThrottleMode::Detents;
    cfg.invert.yaw = false;
    let mut mapper = Mapper::new(&cfg);
    let (axes, _, _) = fields(&mapper.convert(Some(&neutral_stick()), Some(&quadrant(raw(0.4), raw(0.6), &[]))));
    assert_eq!((axes[2], axes[5]), (32768, unit16(neo::shape(-0.2, &cfg.axes.yaw))));
}

#[test]
fn next_profile_combo_and_monitor_report_what_is_sent() {
    let mut cfg = research();
    cfg.combos.next_profile = vec!["s16".into()];
    cfg.shift = Some("s17".into());
    let mut mapper = Mapper::new(&cfg);
    mapper.convert(Some(&neutral_stick()), Some(&quadrant(32768, 32768, &[])));
    mapper.convert(Some(&stick(8192, 8192, 128, 128, 8, &[16, 17, 1, 11])), Some(&quadrant(32768, 32768, &[])));
    assert_eq!(mapper.take_events(), [Event::NextProfile]);
    let monitor = mapper.monitor().unwrap();
    assert_eq!((monitor.shift, monitor.buttons, monitor.pov), (true, 0b1001, 0)); // S1 → NEO 1, S16 → NEO 4, S11 → POV up
}
