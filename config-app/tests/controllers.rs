//! The Controllers page in the window: which controllers take part, the set they make, and following a new choice.
mod common;
use common::{devices_reply, t16000m, FakeApi};
use eframe::egui::{Role, Vec2};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
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

fn tick(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::CheckBox, label).click();
    harness.run_ok();
}

/// The TCA pair taking part, and a T.16000M plugged in beside it.
fn three() -> FakeApi {
    let fake = FakeApi::idle();
    let devices = [neo_daemon::models::tca_quadrant(), neo_daemon::models::tca_stick(), t16000m()];
    fake.reply("/api/devices", Ok(devices_reply(json!({"devices": devices}))));
    fake
}

fn snapshot(size: Vec2, name: &str) {
    let mut harness = open(&three(), size);
    click(&mut harness, "Controllers");
    harness.snapshot(name);
}

#[test]
fn snapshot_at_1600_by_900() {
    snapshot(WIDE, "controllers-1600x900");
}

#[test]
fn snapshot_at_1280_by_800() {
    snapshot(NARROW, "controllers-1280x800");
}

#[test]
fn choosing_controllers_saves_and_follows_their_set() {
    let fake = three();
    let mut harness = open(&fake, WIDE);
    click(&mut harness, "Controllers");
    assert!(harness.query_by_label("Experimental").is_some() && harness.query_by_label("tca-quadrant+tca-stick").is_some());
    assert!(harness.query_by_label("TCA Quadrant Eng 1&2 + TCA Sidestick Airbus").is_some() && harness.query_by_label("Default for this set").is_some());
    // Off automatic, the controllers taking part are listed; then the T.16000M is ticked as well.
    tick(&mut harness, "Automatic: every Thrustmaster joystick that is connected");
    tick(&mut harness, "T.16000M (t16000m)");
    let devices = harness.state().state.settings.draft.as_ref().unwrap().devices.clone();
    assert_eq!(devices, ["tca-quadrant", "tca-stick", "t16000m"]);
    // Saving the new choice asks the daemon which set that is.
    click(&mut harness, "Save settings");
    settle(&mut harness, |_| fake.count("/api/devices") == 2);
    settle(&mut harness, |app| app.state.saved.as_ref().is_some_and(|saved| saved.devices.len() == 3) && !app.state.settings.saving);
    assert!(harness.query_by_label("Saved for this set").is_some());
}

#[test]
fn no_controllers_say_so() {
    let fake = FakeApi::idle();
    fake.reply("/api/devices", Ok(devices_reply(json!({"devices": [], "active": [], "setup": ""}))));
    let mut harness = open(&fake, WIDE);
    click(&mut harness, "Controllers");
    assert!(harness.query_by_label("No controller is connected. Connect your HOTAS, then reopen Configure from the tray icon.").is_some());
    assert!(harness.query_by_label("None").is_some() && harness.query_by_label("\u{2014}").is_some());
}
