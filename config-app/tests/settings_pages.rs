//! The settings pages through egui_kittest against a scripted API: every field and switch edits the draft, the
//! previews follow it, and the save bar saves, discards, resets and tries it in game.
mod common;
use common::{config_reply, live_reply, FakeApi};
use eframe::egui::{Key, Role, Vec2};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use neo_config::launch::Signals;
use neo_common::config::{Lever, ThrottleMode, YawSource};
use neo_config::settings::Field;
use neo_config::App;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const WIDE: Vec2 = Vec2::new(1600.0, 900.0);
pub const NARROW: Vec2 = Vec2::new(1280.0, 800.0);
const SLOW: Duration = Duration::from_secs(3600);

fn fonts() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("Fonts")
}

fn open(fake: &FakeApi, size: Vec2, period: Duration) -> Harness<'static, App> {
    let api = fake.clone();
    let mut harness = Harness::builder().with_size(size).build_eframe(move |cc| App::new(&cc.egui_ctx, Box::new(api), Arc::new(Signals::default()), period, &fonts()));
    settle(&mut harness, |app| app.state.saved.is_some());
    harness
}

fn settle(harness: &mut Harness<'static, App>, done: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        harness.step();
        if done(harness.state()) {
            break;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.run_ok();
}

fn click(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_all_by_label(label).next().unwrap_or_else(|| panic!("no {label}")).click();
    harness.run_ok();
}

fn emulating(fake: &FakeApi) {
    let session = json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true});
    let monitor = json!({"roll": -0.71, "pitch": 0.05, "yaw": -0.18, "slider": 0.5, "levers": [0.62, 0.2], "axes": [9000, 34000, 20000, 40000, 32768, 26000, 32768], "armed": true, "precision": false, "shift": false, "buttons": 0, "pov": 8, "head": [12.0, -4.0]});
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "monitor": monitor, "levers": [0.62, 0.2]}))));
}

/// Every settings page against a running session, at one of the design's sizes.
fn snapshot_pages(size: Vec2, suffix: &str) {
    let fake = FakeApi::idle();
    let mut config = neo_daemon::config::defaults();
    config.throttle.mode = neo_common::config::ThrottleMode::Detents;
    config.camera.full_after_hold = true;
    config.camera.ramp_ms = 400;
    config.head_tracking.enabled = true;
    config.precision.source = Some("s5".into());
    config.combos.pause = vec!["s7".into(), "s8".into()];
    fake.reply("/api/config", Ok(config_reply(json!({"config": config}))));
    emulating(&fake);
    let mut harness = open(&fake, size, Duration::from_millis(20));
    settle(&mut harness, |app| app.state.live());
    for (page, name) in [("Flight axes", "axes"), ("Throttle", "throttle"), ("Camera & head", "camera"), ("Buttons & helpers", "helpers")] {
        click(&mut harness, page);
        harness.run_ok();
        harness.snapshot(format!("settings-{name}-{suffix}"));
    }
}

#[test]
fn snapshots_at_1600_by_900() {
    snapshot_pages(WIDE, "1600x900");
}

#[test]
fn snapshots_at_1280_by_800() {
    snapshot_pages(NARROW, "1280x800");
}
/// Replace a number field's text: focus it, clear it, type.
fn fill(harness: &mut Harness<'static, App>, label: &str, text: &str) {
    harness.get_by_role_and_label(Role::TextInput, label).focus();
    harness.run_ok();
    harness.key_press(Key::End);
    for _ in 0..8 {
        harness.key_press(Key::Backspace);
    }
    harness.get_by_role_and_label(Role::TextInput, label).type_text(text);
    harness.run_ok();
}

/// Nudge a slider one step with the arrow keys.
fn nudge(harness: &mut Harness<'static, App>, label: &str, key: Key) {
    harness.get_by_label(label).focus();
    harness.run_ok();
    harness.key_press(key);
    harness.run_ok();
}

fn draft(harness: &Harness<'static, App>) -> neo_common::config::Config {
    harness.state().state.settings.draft.clone().unwrap()
}

fn message(harness: &Harness<'static, App>) -> String {
    harness.state().state.settings.message.clone().map(|(text, _)| text).unwrap_or_default()
}

#[test]
fn flight_axes_edit_the_draft_and_the_save_bar_saves_it() {
    let fake = FakeApi::idle();
    fake.reply("PUT /api/preview", Ok(json!({"preview": true}).to_string()));
    fake.reply("DELETE /api/preview", Ok(json!({"preview": false}).to_string()));
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "Flight axes");
    // The test input stands in for the stick without a session.
    nudge(&mut harness, "Test stick input, percent", Key::ArrowRight);
    assert_eq!(harness.state().state.settings.test_input, 0.41);
    fill(&mut harness, "Deadzone", "10");
    assert_eq!(draft(&harness).axes.roll.deadzone, 0.1);
    nudge(&mut harness, "Deadzone, percent", Key::ArrowRight);
    nudge(&mut harness, "Smoothing, percent", Key::ArrowRight);
    assert_eq!((draft(&harness).axes.roll.deadzone, draft(&harness).axes.roll.smoothing), (0.11, 0.05));
    // A click on the track sets the slider where it lands: the middle of the curve slider is 0.
    fill(&mut harness, "Curve", "40");
    harness.get_by_label("Curve, percent").click();
    harness.run_ok();
    assert_eq!(draft(&harness).axes.roll.curve, 0.0);
    harness.snapshot("settings-axes-edited-1600x900");
    click(&mut harness, "Copy roll to pitch");
    assert_eq!(draft(&harness).axes.pitch.deadzone, 0.11);
    click(&mut harness, "Yaw");
    click(&mut harness, "Copy yaw to roll and pitch");
    click(&mut harness, "Pitch");
    click(&mut harness, "Reset pitch");
    assert_eq!(draft(&harness).axes.pitch.deadzone, 0.04);
    for (label, source) in [("No yaw", YawSource::Off), ("Yaw from the lever difference", YawSource::LeverDifferential), ("Yaw from the twist", YawSource::Twist)] {
        click(&mut harness, label);
        assert_eq!(draft(&harness).yaw_source, source);
    }
    for label in ["Roll", "Pitch", "Yaw"] {
        harness.get_by_role_and_label(Role::CheckBox, label).click();
        harness.run_ok();
    }
    assert!(draft(&harness).invert.roll && draft(&harness).invert.pitch && draft(&harness).invert.yaw);
    // A problem blocks saving and trying in game, and shows where it is.
    fill(&mut harness, "Curve", "500");
    click(&mut harness, "Save settings");
    assert_eq!(message(&harness), "Fix the highlighted fields, then save.");
    assert!(harness.query_by_label("Enter a number from -100 to 100.").is_some());
    harness.get_by_role_and_label(Role::CheckBox, "Try in game").click();
    harness.run_ok();
    assert!(!harness.state().state.previewing());
    fill(&mut harness, "Curve", "20");
    // Try in game, then Discard: back to the saved profile in game and in the draft.
    harness.get_by_role_and_label(Role::CheckBox, "Try in game").click();
    settle(&mut harness, |app| !app.state.settings.sending);
    assert!(harness.state().state.previewing() && fake.count("/api/preview") == 1);
    click(&mut harness, "Discard");
    settle(&mut harness, |_| fake.count("/api/preview") == 2);
    assert_eq!((draft(&harness), message(&harness)), (neo_daemon::config::defaults(), "Changes discarded.".to_string()));
    click(&mut harness, "Reset to defaults");
    fill(&mut harness, "Curve", "15");
    click(&mut harness, "Save settings");
    settle(&mut harness, |app| !app.state.settings.saving);
    // The tuned axis is still pitch.
    assert_eq!((harness.state().state.saved.as_ref().unwrap().axes.pitch.curve, message(&harness)), (0.15, "Saved.".to_string()));
}

#[test]
fn throttle_detents_levers_and_split_high_g() {
    let fake = FakeApi::idle();
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "Throttle");
    nudge(&mut harness, "Test lever travel, percent", Key::ArrowLeft);
    assert_eq!(harness.state().state.settings.test_lever, 0.49);
    click(&mut harness, "Detent throttle");
    assert_eq!(draft(&harness).throttle.mode, ThrottleMode::Detents);
    fill(&mut harness, "Idle (brake below)", "12");
    fill(&mut harness, "Cruise band \u{b1}", "4");
    assert_eq!((draft(&harness).throttle.idle, draft(&harness).throttle.band), (0.12, 0.04));
    harness.snapshot("settings-throttle-edited-1600x900");
    click(&mut harness, "Right lever");
    assert_eq!(draft(&harness).throttle_lever, Lever::Right);
    harness.get_by_role_and_label(Role::CheckBox, "Invert throttle").click();
    harness.run_ok();
    harness.get_by_role_and_label(Role::CheckBox, "Hold NEO 5 (High-G) while one quadrant lever is forward and the other back").click();
    harness.run_ok();
    assert!(draft(&harness).invert.throttle && !draft(&harness).split_high_g.enabled);
    fill(&mut harness, "Forward lever at least", "80");
    nudge(&mut harness, "Other lever at most, percent", Key::ArrowRight);
    assert_eq!((draft(&harness).split_high_g.forward, draft(&harness).split_high_g.back), (0.8, 0.26));
    click(&mut harness, "Linear throttle");
    // A wrong detent order is reported once for the group.
    click(&mut harness, "Detent throttle");
    fill(&mut harness, "Boost from", "40");
    click(&mut harness, "Save settings");
    assert!(harness.query_by_label("Needs idle < cruise \u{2212} band and cruise + band < boost.").is_some());
    // While emulating, the live lever drives the page and the detents can be captured.
    emulating(&fake);
    let mut harness = open(&fake, WIDE, Duration::from_millis(5));
    settle(&mut harness, |app| app.state.live());
    click(&mut harness, "Throttle");
    click(&mut harness, "Detent throttle");
    for (label, field) in [("Capture Idle (brake below)", Field::Idle), ("Capture CL (cruise centre)", Field::Cruise), ("Capture Boost from", Field::Boost)] {
        click(&mut harness, label);
        assert_eq!(harness.state().state.settings.fields[&field], "62");
    }
}

#[test]
fn split_levers_show_their_zones_live() {
    for levers in [[0.8, 0.1], [0.5, 0.5]] {
        let fake = FakeApi::idle();
        let session = json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true});
        fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "levers": levers}))));
        let mut harness = open(&fake, NARROW, Duration::from_millis(5));
        settle(&mut harness, |app| app.state.levers.is_some());
        click(&mut harness, "Throttle");
        harness.run_ok();
        assert!(harness.query_all_by_label_contains("Left lever").next().is_some());
    }
}

#[test]
fn camera_and_head_tracking() {
    let fake = FakeApi::idle();
    let session = json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true});
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "pressed": ["hat-up"]}))));
    let mut harness = open(&fake, WIDE, Duration::from_millis(5));
    settle(&mut harness, |app| app.state.settings.hat_since.is_some());
    click(&mut harness, "Camera & head");
    fill(&mut harness, "Strength", "50");
    fill(&mut harness, "Ramp", "300");
    assert_eq!((draft(&harness).camera_strength, draft(&harness).camera.ramp_ms), (0.5, 300));
    harness.get_by_role_and_label(Role::CheckBox, "Full strength after a long press: hold the hat to look all the way round").click();
    harness.run_ok();
    click(&mut harness, "Camera from the hat and the spare lever");
    assert!(draft(&harness).camera.lever && !draft(&harness).split_high_g.enabled);
    click(&mut harness, "Throttle");
    click(&mut harness, "Right lever");
    click(&mut harness, "Camera & head");
    click(&mut harness, "Camera from the hat");
    harness.get_by_role_and_label(Role::CheckBox, "Look around with your head: OpenTrack moves the NEO camera").click();
    harness.run_ok();
    for label in ["Invert L / R", "Invert U / D"] {
        harness.get_by_role_and_label(Role::CheckBox, label).click();
        harness.run_ok();
    }
    let head = draft(&harness).head_tracking;
    assert!(head.enabled && head.invert_yaw && head.invert_pitch);
    fill(&mut harness, "UDP port", "5000");
    fill(&mut harness, "Full look L / R", "500");
    click(&mut harness, "Save settings");
    assert!(harness.query_by_label("Enter a number from 5 to 180.").is_some());
    // Without a session, head tracking waits for emulation.
    let idle = FakeApi::idle();
    let mut config = neo_daemon::config::defaults();
    config.head_tracking.enabled = true;
    config.hat = neo_common::config::HatMode::Mapped;
    idle.reply("/api/config", Ok(config_reply(json!({"config": config}))));
    let mut harness = open(&idle, NARROW, SLOW);
    click(&mut harness, "Camera & head");
    assert!(harness.query_by_label("STARTS WITH EMULATION").is_some() && harness.query_by_label("HAT MAPPED AS BUTTONS").is_some());
}

#[test]
fn helpers_pick_controls_by_pressing_them() {
    let fake = FakeApi::idle();
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "Buttons & helpers");
    // Without a session the prompt says to start emulation.
    click(&mut harness, "Press a control\u{2026} (precision)");
    assert!(harness.query_by_label("Start emulation, then press the controls on the TCA.").is_some());
    click(&mut harness, "Cancel (precision)");
    click(&mut harness, "Record combo\u{2026} (pause)");
    click(&mut harness, "Done (pause)");
    assert_eq!(message(&harness), "Unsaved: pause combo off.");
    harness.get_by_role_and_label(Role::CheckBox, "Hold the throttle at centre until the active lever or slider reaches it").click();
    harness.run_ok();
    assert!(!draft(&harness).safe_start);
    fill(&mut harness, "Long press", "600");
    fill(&mut harness, "Stick scale while held", "30");
    assert_eq!((draft(&harness).timing.long_ms, draft(&harness).precision.scale), (600, 0.3));
    // With a session, presses pick the controls and fill the timeline.
    let live = FakeApi::idle();
    let session = json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true});
    live.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "monitor": common::monitor()}))));
    let mut harness = open(&live, WIDE, Duration::from_millis(5));
    click(&mut harness, "Buttons & helpers");
    for (start, kind, key) in [("Press a control\u{2026} (precision)", "precision", "s5"), ("Press a control\u{2026} (shift)", "shift", "s14")] {
        click(&mut harness, start);
        assert!(harness.query_by_label_contains(&format!("to hold for {}", if kind == "precision" { "precision aiming" } else { "the shift layer" })).is_some());
        live.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "monitor": common::monitor(), "pressed": [key]}))));
        settle(&mut harness, |app| app.state.settings.capture.is_none());
        live.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "monitor": common::monitor()}))));
        settle(&mut harness, |app| app.state.map.held.is_empty());
    }
    assert_eq!((draft(&harness).precision.source.as_deref(), draft(&harness).shift.as_deref()), (Some("s5"), Some("s14")));
    click(&mut harness, "Record combo\u{2026} (next-profile)");
    assert!(harness.query_by_label("Press and release the controls for the combo (up to 4), then choose Done.").is_some());
    live.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "monitor": common::monitor(), "pressed": ["s1", "s2"]}))));
    settle(&mut harness, |app| app.state.settings.capture.as_ref().is_some_and(|capture| capture.keys.len() == 2));
    click(&mut harness, "Done (next-profile)");
    assert_eq!(draft(&harness).combos.next_profile, ["s1", "s2"]);
    // A long hold reads as a long press on the timeline.
    live.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "monitor": common::monitor(), "pressed": ["s9"]}))));
    settle(&mut harness, |app| app.state.settings.latest().is_some_and(|(key, presses)| key == "s9" && app.state.now - presses[0].start > 700.0));
    assert!(harness.query_all_by_label_contains("S9 \u{b7} long press").next().is_some());
    click(&mut harness, "Clear the next-profile combo");
    click(&mut harness, "Clear the precision control");
    click(&mut harness, "Clear the shift control");
    assert!(draft(&harness).combos.next_profile.is_empty() && draft(&harness).precision.source.is_none() && draft(&harness).shift.is_none());
}

#[test]
fn leaving_the_settings_or_closing_the_window_ends_a_preview() {
    let fake = FakeApi::idle();
    fake.reply("PUT /api/preview", Ok(json!({"preview": true}).to_string()));
    fake.reply("DELETE /api/preview", Ok(json!({"preview": false}).to_string()));
    let session = json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true});
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "preview": true}))));
    let mut harness = open(&fake, WIDE, Duration::from_millis(5));
    click(&mut harness, "Throttle");
    harness.get_by_role_and_label(Role::CheckBox, "Try in game").click();
    settle(&mut harness, |app| !app.state.settings.sending);
    click(&mut harness, "Mapping");
    settle(&mut harness, |_| fake.count("/api/preview") == 2);
    assert_eq!(message(&harness), "Unsaved changes \u{b7} preview ended when you left the settings.");
    click(&mut harness, "Throttle");
    harness.get_by_role_and_label(Role::CheckBox, "Try in game").click();
    settle(&mut harness, |app| !app.state.settings.sending);
    harness.input_mut().viewports.entry(eframe::egui::ViewportId::ROOT).or_default().events.push(eframe::egui::ViewportEvent::Close);
    harness.step();
    assert!(!harness.state().state.previewing());
    drop(harness);
    assert_eq!(fake.count("/api/preview"), 4);
}