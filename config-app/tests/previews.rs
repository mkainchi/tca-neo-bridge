//! The settings previews as numbers: the same maths and texts the page used, through the daemon's own functions.
use neo_common::api::Monitor;
use neo_common::config::{Camera, Config, HatMode, ThrottleMode, Timing};
use neo_config::previews::*;

fn defaults() -> Config {
    neo_daemon::config::defaults()
}

fn monitor(precision: bool, shift: bool, armed: bool) -> Monitor {
    Monitor { roll: 0.0, pitch: 0.0, yaw: 0.0, slider: 0.0, levers: [0.0, 0.0], axes: [32768; 7], armed, precision, shift, buttons: 0, pov: 8, head: None }
}

#[test]
fn numbers_read_as_the_page_wrote_them() {
    assert_eq!([0.4, -0.4, 0.0, -0.001, 0.004].map(signed), ["+0.40", "\u{2212}0.40", " 0.00", " 0.00", " 0.00"]);
    assert_eq!([0.0, 0.5, 0.9].map(lag), [0, 10, 90]);
    assert_eq!([12.4, -3.6, 0.2].map(degrees), ["+12\u{b0}", "\u{2212}4\u{b0}", "0\u{b0}"]);
    assert_eq!([0, 32768, 65535].map(axis_value), [-1.0, 32768.0 / 65535.0 * 2.0 - 1.0, 1.0]);
}

#[test]
fn the_throttle_reads_linear_or_through_the_detents() {
    let mut throttle = defaults().throttle;
    assert_eq!(throttle_out(0.75, &throttle), 0.5);
    assert_eq!(zone(0.5, &throttle), (None, "75%".into()));
    throttle.mode = ThrottleMode::Detents;
    let cases = [(0.0, Some(Zone::Brake), "Full brake"), (0.3, Some(Zone::Brake), "Braking"), (0.5, Some(Zone::Cruise), "Cruise"), (0.7, Some(Zone::Boost), "Boosting"), (1.0, Some(Zone::Boost), "Full boost")];
    for (travel, area, name) in cases {
        let out = throttle_out(travel, &throttle);
        assert_eq!(zone(out, &throttle), (area, name.to_string()), "{travel}");
    }
    assert_eq!([None, Some(Zone::Cruise), Some(Zone::Brake), Some(Zone::Boost)].map(zone_tone), [Tone::Plain, Tone::Ok, Tone::Warn, Tone::Accent]);
}

#[test]
fn curves_and_ramps_follow_the_settings() {
    let tuning = defaults().axes.roll;
    let points = curve(&tuning);
    assert_eq!((points.len(), points[0], points[60].1, points[120]), (121, (-1.0, -1.0), 0.0, (1.0, 1.0)));
    let instant = Camera { ramp_ms: 0, full_after_hold: false, lever: false };
    assert!(ramp(&instant, 0.5, 500).iter().all(|(_, value)| *value == 0.5));
    let slow = Camera { ramp_ms: 1000, full_after_hold: true, lever: false };
    let points = ramp(&slow, 0.5, 600);
    assert_eq!((points[0], points[20], points[24], points.last().copied()), ((0, 0.0), (500, 0.25), (600, 1.0), Some((1500, 1.0))));
}

#[test]
fn head_samples_move_the_camera_through_the_head_settings() {
    let mut tracking = defaults().head_tracking;
    let [x, y] = head_camera([45.0, 30.0], &tracking);
    assert_eq!((x, y), (1.0, -1.0));
    tracking.invert_yaw = true;
    tracking.invert_pitch = true;
    assert_eq!(head_camera([45.0, 30.0], &tracking), [-1.0, 1.0]);
}

#[test]
fn split_levers_show_when_the_rule_holds() {
    let mut rule = defaults().split_high_g;
    assert!(split_active([0.8, 0.1], &rule) && split_active([0.1, 0.8], &rule) && !split_active([0.5, 0.1], &rule));
    assert_eq!(lever_text([0.8, 0.1], &rule), "Left lever 80% \u{b7} Right lever 10% \u{b7} High-G active");
    assert_eq!(lever_state(&rule, Some([0.8, 0.1])), ("High-G active \u{b7} NEO 5 held".into(), Tone::Warn));
    assert_eq!(lever_state(&rule, None), ("Not split".into(), Tone::Plain));
    rule.enabled = false;
    assert_eq!(lever_text([0.8, 0.1], &rule), "Left lever 80% \u{b7} Right lever 10%");
    assert_eq!(lever_state(&rule, Some([0.8, 0.1])), ("Rule off".into(), Tone::Plain));
}

#[test]
fn presses_read_as_taps_long_presses_and_double_taps() {
    let timing = Timing { pulse_ms: 80, long_ms: 500, double_ms: 250, repeat_ms: 120 };
    let press = |start: f64, end: Option<f64>| Press { start, end };
    // A tap, then a second tap soon after (a double tap), then a long press, then a press still being held.
    let presses = [press(0.0, Some(100.0)), press(200.0, Some(260.0)), press(1000.0, Some(1700.0)), press(2000.0, None)];
    assert_eq!(gestures(&presses, &timing, 2100.0), [(Gesture::Double, 200.0, 280.0), (Gesture::Long, 1500.0, 1700.0)]);
    // A slow second tap is a tap of its own.
    let slow = [press(0.0, Some(100.0)), press(600.0, Some(650.0))];
    assert_eq!(gestures(&slow, &timing, 700.0), [(Gesture::Tap, 100.0, 180.0), (Gesture::Tap, 650.0, 730.0)]);
    // A press held past the long-press time reads as long while still held.
    assert_eq!(gestures(&[press(0.0, None)], &timing, 800.0), [(Gesture::Long, 500.0, 800.0)]);
    assert_eq!(press_text(None, &[], &timing, 0.0), "Press a TCA control: the last 3 seconds are drawn against the long-press and double-tap times.");
    assert_eq!(press_text(Some("s5"), &[press(0.0, None)], &timing, 120.0), "S5 \u{b7} holding 120 ms\u{2026}");
    assert_eq!(press_text(Some("s5"), &[press(0.0, None)], &timing, 900.0), "S5 \u{b7} long press: held 900 ms (long press from 500 ms)");
    assert_eq!(press_text(Some("s5"), &presses[..2], &timing, 400.0), "S5 \u{b7} double tap: second tap within 250 ms");
    assert_eq!(press_text(Some("q2"), &slow, &timing, 700.0), "Q2 \u{b7} tap: 50 ms");
}

#[test]
fn helper_states_follow_the_draft_and_the_session() {
    let mut config = defaults();
    let held = monitor(true, true, true);
    let idle = monitor(false, false, false);
    assert_eq!(precision_state(&config, Some(&held)), ("Precision off".into(), Tone::Plain));
    assert_eq!(shift_state(&config, Some(&held)), ("Shift off".into(), Tone::Plain));
    config.precision.source = Some("s5".into());
    config.shift = Some("s14".into());
    assert_eq!(precision_state(&config, Some(&held)), ("Precision held \u{b7} 50%".into(), Tone::Accent));
    assert_eq!(precision_state(&config, None), ("Ready".into(), Tone::Plain));
    assert_eq!(shift_state(&config, Some(&held)), ("Shift held".into(), Tone::Warn));
    assert_eq!(shift_state(&config, Some(&idle)), ("Ready".into(), Tone::Plain));
    assert_eq!(safe_state(&config, None), ("Starts with emulation".into(), Tone::Plain));
    assert_eq!(safe_state(&config, Some(&held)), ("Throttle armed".into(), Tone::Ok));
    assert_eq!(safe_state(&config, Some(&idle)), ("Waiting for centre".into(), Tone::Warn));
    config.safe_start = false;
    assert_eq!(safe_state(&config, Some(&held)), ("Off".into(), Tone::Plain));
    let keys = |list: &[&str]| list.iter().map(|key| key.to_string()).collect::<Vec<_>>();
    assert_eq!(combo_state(&config, &keys(&["s1"])), ("No combo held".into(), Tone::Plain));
    config.combos.pause = keys(&["s7", "s8"]);
    config.combos.next_profile = keys(&["s9"]);
    assert_eq!(combo_state(&config, &keys(&["s7"])), ("pause 1/2".into(), Tone::Warn));
    assert_eq!(combo_state(&config, &keys(&["s7", "s8"])), ("Pause combo held".into(), Tone::Ok));
    assert_eq!(combo_state(&config, &keys(&["s9"])), ("Next-profile combo held".into(), Tone::Ok));
    assert_eq!(hat_state(HatMode::Mapped, Some(5.0)), ("Hat mapped as buttons".into(), Tone::Plain));
    assert_eq!(hat_state(HatMode::Camera, Some(250.4)), ("Hat held \u{b7} 250 ms".into(), Tone::Ok));
    assert_eq!(hat_state(HatMode::Camera, None), ("Hat released".into(), Tone::Plain));
    let mut tracking = config.head_tracking;
    assert_eq!(head_state(&tracking, Some([1.0, 2.0]), true), ("Off".into(), Tone::Plain));
    tracking.enabled = true;
    assert_eq!(head_state(&tracking, Some([1.0, 2.0]), true), ("Receiving from OpenTrack".into(), Tone::Ok));
    assert_eq!(head_state(&tracking, None, true), ("Waiting for OpenTrack on UDP port 4242\u{2026}".into(), Tone::Warn));
    assert_eq!(head_state(&tracking, None, false), ("Starts with emulation".into(), Tone::Warn));
}
