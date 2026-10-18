use neo_daemon::config::{self, Direction, Lever};
use neo_daemon::neo::{self, centered, input_report, unit16, Mapper, ThrottleSelector};
use neo_daemon::{protocol, validation};

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
    for (button, neo) in [(1, 5), (2, 5), (3, 6), (4, 6), (5, 9), (6, 6), (7, 7), (8, 8)] {
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
    cfg.deadzone = 0.0;
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
    cfg.deadzone = 0.0;
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
