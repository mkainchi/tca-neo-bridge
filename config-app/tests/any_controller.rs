//! The Mapping page with controllers other than the TCA pair: a T.16000M stick and a TWCS throttle.
mod common;
use common::{devices_reply, t16000m, twcs, FakeApi};
use eframe::egui::{Key, Vec2};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use neo_common::config::HatMode;
use neo_config::launch::Signals;
use neo_config::App;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const WIDE: Vec2 = Vec2::new(1600.0, 900.0);
const NARROW: Vec2 = Vec2::new(1280.0, 800.0);
const SLOW: Duration = Duration::from_secs(3600);

fn fonts() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("Fonts")
}

fn open(fake: &FakeApi, size: Vec2) -> Harness<'static, App> {
    let api = fake.clone();
    let mut harness = Harness::builder().with_size(size).build_eframe(move |cc| App::new(&cc.egui_ctx, Box::new(api), Arc::new(Signals::default()), SLOW, &fonts()));
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

/// The stick and the throttle taking part, with the daemon's default mapping for that set.
fn stick_and_throttle() -> FakeApi {
    let fake = FakeApi::idle();
    let devices = [t16000m(), twcs()];
    let profile = neo_daemon::config::default_profile(&devices);
    fake.reply("/api/devices", Ok(devices_reply(json!({"devices": devices, "active": ["t16000m", "twcs"], "setup": "t16000m+twcs", "profile": profile, "default": profile}))));
    fake
}

#[test]
fn snapshot_at_1600_by_900() {
    let mut harness = open(&stick_and_throttle(), WIDE);
    harness.snapshot("mapping-generic-1600x900");
}

#[test]
fn snapshot_at_1280_by_800() {
    let mut harness = open(&stick_and_throttle(), NARROW);
    harness.snapshot("mapping-generic-1280x800");
}

#[test]
fn any_controller_maps_its_buttons_and_hats() {
    let fake = stick_and_throttle();
    let mut harness = open(&fake, WIDE);
    // A hat row per controller with a hat; no TCA rows.
    click(&mut harness, "TWCS Throttle button 3");
    assert_eq!(harness.state().state.map.selected.as_deref(), Some("twcs/b3"));
    // The stick's hat drives the camera, so its directions wait; the throttle's hat maps like buttons.
    assert_eq!(harness.query_all_by_label("Set the camera hat to Mapped to map it").count(), 4);
    click(&mut harness, "TWCS Throttle hat up");
    assert_eq!(harness.state().state.map.selected.as_deref(), Some("twcs/hat-up"));
    // Choosing the other hat for the camera saves at once and swaps which hat waits.
    click(&mut harness, "TWCS Throttle hat drives the camera");
    settle(&mut harness, |app| app.state.saved.as_ref().is_some_and(|saved| saved.camera_hat.as_deref() == Some("twcs")));
    assert_eq!(harness.state().state.map.selected, None);
    assert_eq!(harness.state().state.map.note.as_deref(), Some("TWCS Throttle hat drives the camera; its mappings are kept but inactive."));
    click(&mut harness, "T.16000M hat up");
    assert_eq!(harness.state().state.map.selected.as_deref(), Some("t16000m/hat-up"));
    click(&mut harness, "Hat mapped");
    settle(&mut harness, |app| app.state.saved.as_ref().is_some_and(|saved| saved.hat == HatMode::Mapped));
    let sent: neo_common::config::Config = serde_json::from_str(&fake.bodies().last().unwrap().1).unwrap();
    assert_eq!((sent.setups["t16000m+twcs"].camera_hat.as_deref(), sent.setups["t16000m+twcs"].hat), (Some("twcs"), HatMode::Mapped));
}

#[test]
fn one_hat_or_none_offers_no_choice() {
    for (devices, text) in [(vec![t16000m()], "T.16000M"), (vec![neo_daemon::models::tca_quadrant()], "No hat")] {
        let fake = FakeApi::idle();
        let ids: Vec<String> = devices.iter().map(|device| device.id.clone()).collect();
        fake.reply("/api/devices", Ok(devices_reply(json!({"devices": devices, "active": ids, "setup": ids.join("+")}))));
        let harness = open(&fake, WIDE);
        assert!(harness.query_all_by_label(text).count() > 0, "{text}");
    }
    let empty = FakeApi::idle();
    empty.reply("/api/devices", Ok(devices_reply(json!({"devices": [], "active": [], "setup": ""}))));
    let harness = open(&empty, WIDE);
    assert!(harness.query_by_label("No controller found. Connect your HOTAS, or tick it on the Controllers page, then reopen Configure from the tray icon.").is_some());
}

#[test]
fn a_low_axis_picker_opens_upward_within_the_window() {
    let mut harness = open(&stick_and_throttle(), NARROW);
    click(&mut harness, "Camera & head");
    click(&mut harness, "Camera from the hat and a lever");
    harness.get_by_label("Camera lever").focus();
    harness.run_ok();
    // Too low on the page for the list below, so it opens above; down to the last axis of both controllers, the
    // highlighted row stays in the window.
    harness.key_press(Key::ArrowDown);
    harness.run_ok();
    for _ in 0..30 {
        harness.key_press(Key::ArrowDown);
        harness.run_ok();
    }
    let last = "TWCS Throttle \u{b7} slider";
    assert!(harness.get_by_label(last).rect().bottom() <= NARROW.y);
    harness.snapshot("any-controller-axis-list-1280x800");
    harness.key_press(Key::Enter);
    harness.run_ok();
    let lever = harness.state().state.settings.draft.clone().unwrap().camera_lever.unwrap();
    assert_eq!((lever.device.as_str(), lever.axis.as_str()), ("twcs", "slider"));
}