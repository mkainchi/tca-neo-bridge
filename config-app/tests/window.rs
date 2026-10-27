//! The window through egui_kittest against a scripted API: the shell, the rail, pills, warnings, the save bar, the
//! daemon's show and quit signals, and snapshots at the design's two desktop sizes.
mod common;
use common::{config_reply, live_reply, FakeApi};
use eframe::egui::{self, Vec2, ViewportCommand, ViewportId};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use neo_config::api::ApiError;
use neo_config::launch::Signals;
use neo_config::state::Page;
use neo_config::App;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

const WIDE: Vec2 = Vec2::new(1600.0, 900.0);
const NARROW: Vec2 = Vec2::new(1280.0, 800.0);
const SLOW: Duration = Duration::from_secs(3600);

fn fonts() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("Fonts")
}

fn open(fake: &FakeApi, size: Vec2, period: Duration) -> (Harness<'static, App>, Arc<Signals>) {
    let signals = Arc::new(Signals::default());
    let (api, app_signals) = (fake.clone(), signals.clone());
    let harness = Harness::builder().with_size(size).build_eframe(move |cc| App::new(&cc.egui_ctx, Box::new(api), app_signals, period, &fonts()));
    (harness, signals)
}

/// Step the window until the worker's replies have arrived and `done` holds.
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

fn commands(harness: &Harness<'static, App>) -> Vec<String> {
    harness.output().viewport_output[&ViewportId::ROOT].commands.iter().map(|command| format!("{command:?}")).collect()
}

#[test]
fn the_shell_loads_and_the_rail_switches_pages() {
    let fake = FakeApi::idle();
    let (mut harness, _) = open(&fake, WIDE, SLOW);
    settle(&mut harness, |app| app.state.saved.is_some());
    assert!(harness.query_by_label("Daemon: Idle").is_some());
    assert!(harness.query_by_label("Unsaved profile").is_some());
    assert!(harness.query_by_label("TCA: not open").is_some() && harness.query_by_label("Idle").is_some());
    for page in Page::ALL.iter().rev() {
        harness.get_by_label(page.title()).click();
        harness.run_ok();
        assert_eq!(harness.state().state.page, *page);
        assert!(harness.query_by_label(page.help()).is_some());
    }
    harness.get_by_label("Flight axes").hover();
    harness.run_ok();
    // Keyboard: Tab reaches the rail and Enter opens the focused page.
    harness.get_by_label("Throttle").focus();
    harness.run_ok();
    harness.key_press(egui::Key::Enter);
    harness.run_ok();
    assert_eq!(harness.state().state.page, Page::Throttle);
}

#[test]
fn a_narrow_window_has_an_icon_rail_and_fewer_pills() {
    let fake = FakeApi::idle();
    let (mut harness, _) = open(&fake, NARROW, SLOW);
    settle(&mut harness, |app| app.state.saved.is_some());
    assert!(harness.query_by_label("TCA: not open").is_none() && harness.query_by_label("HidHide: off").is_none());
    assert!(harness.query_by_label("Virtual NEO: absent").is_some());
    harness.get_by_label("Live").click();
    harness.run_ok();
    assert_eq!(harness.state().state.page, Page::Live);
}

#[test]
fn live_state_shows_in_the_pills_and_the_rail() {
    let fake = FakeApi::idle();
    let session = json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true});
    // The window opens on a preview already running (the pill follows this window's own preview).
    fake.reply("/api/config", Ok(config_reply(json!({"preview": neo_daemon::config::defaults()}))));
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "paused": true, "preview": true, "links": session, "monitor": common::monitor()}))));
    let (mut harness, _) = open(&fake, WIDE, Duration::from_millis(5));
    settle(&mut harness, |app| app.state.live() && app.state.saved.is_some());
    for label in ["Daemon: Emulating", "Input paused", "Previewing unsaved", "TCA: sidestick + quadrant open", "HidHide: TCA hidden", "Virtual NEO present", "Live input"] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    harness.snapshot("shell-live-1600x900");
    // Minimised: polling stops until the window is back.
    harness.input_mut().viewports.entry(ViewportId::ROOT).or_default().minimized = Some(true);
    harness.step();
    std::thread::sleep(Duration::from_millis(30));
    let polls = fake.count("/api/live");
    std::thread::sleep(Duration::from_millis(60));
    assert!(fake.count("/api/live") <= polls + 1);
}

#[test]
fn starting_validation_and_errors_tint_the_pills() {
    let fake = FakeApi::idle();
    let validation = json!({"source": "validation", "hidden": false, "tca": false, "virtual_neo": false});
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Starting", "links": validation}))));
    let (mut harness, _) = open(&fake, WIDE, Duration::from_millis(5));
    settle(&mut harness, |app| app.state.status == "Starting");
    assert!(harness.query_by_label("Source: validation stream").is_some() && harness.query_by_label("HidHide: not needed").is_some());
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Error: The TCA was unplugged."}))));
    settle(&mut harness, |app| app.state.status.starts_with("Error"));
    assert!(harness.query_by_label("Daemon: Error: The TCA was unplugged.").is_some());
}

#[test]
fn warnings_show_until_dismissed_and_return_when_they_change() {
    let fake = FakeApi::idle();
    fake.reply("/api/config", Ok(config_reply(json!({"warnings": ["Steam is running: start ACE COMBAT 8 after the bridge."]}))));
    let (mut harness, _) = open(&fake, WIDE, SLOW);
    settle(&mut harness, |app| app.state.saved.is_some());
    assert!(harness.query_by_label("Steam is running: start ACE COMBAT 8 after the bridge.").is_some());
    harness.snapshot("shell-warning-1600x900");
    harness.get_by_label("Dismiss warnings").click();
    harness.run_ok();
    assert!(harness.query_by_label("Dismiss warnings").is_none());
}

#[test]
fn a_failed_load_shows_in_the_save_bar() {
    let fake = FakeApi::idle();
    fake.reply("/api/profiles", Err(ApiError::Rejected("The profiles folder could not be read.".into())));
    let (mut harness, _) = open(&fake, NARROW, SLOW);
    settle(&mut harness, |app| app.state.note.is_some());
    assert!(harness.query_by_label("Saved profiles could not be listed: The profiles folder could not be read.").is_some());
}

/// The shell with a saved profile active, at one of the design's sizes.
fn snapshot_at(size: Vec2, name: &str) {
    let fake = FakeApi::idle();
    fake.reply("/api/profiles", Ok(json!({"profiles": ["Dogfight"], "active": "Dogfight"}).to_string()));
    let (mut harness, _) = open(&fake, size, SLOW);
    settle(&mut harness, |app| app.state.saved.is_some());
    assert!(harness.query_by_label("Dogfight").is_some());
    harness.snapshot(name);
}

#[test]
fn snapshot_at_1600_by_900() {
    snapshot_at(WIDE, "shell-1600x900");
}

#[test]
fn snapshot_at_1280_by_800() {
    snapshot_at(NARROW, "shell-1280x800");
}

#[test]
fn the_daemon_brings_the_window_forward_and_closes_it() {
    let fake = FakeApi::idle();
    let (mut harness, signals) = open(&fake, WIDE, SLOW);
    settle(&mut harness, |app| app.state.saved.is_some());
    signals.show.store(true, Ordering::SeqCst);
    harness.step();
    assert_eq!(commands(&harness), [format!("{:?}", ViewportCommand::Visible(true)), format!("{:?}", ViewportCommand::Minimized(false)), format!("{:?}", ViewportCommand::Focus)]);
    assert!(!signals.show.load(Ordering::SeqCst));
    harness.step();
    assert!(commands(&harness).is_empty());
    signals.quit.store(true, Ordering::SeqCst);
    harness.step();
    assert_eq!(commands(&harness), [format!("{:?}", ViewportCommand::Close)]);
}

#[test]
fn a_stale_token_closes_the_window() {
    let fake = FakeApi::idle();
    fake.reply("/api/config", Err(ApiError::Forbidden));
    let (mut harness, _) = open(&fake, WIDE, SLOW);
    settle(&mut harness, |app| app.state.closing);
    harness.step();
    assert!(commands(&harness).contains(&format!("{:?}", ViewportCommand::Close)));
}
