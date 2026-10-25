//! The Mapping page through egui_kittest against a scripted API: diagrams, selection, building and recording a
//! mapping, inspecting NEO targets, layers and actions, the hat mode, live presses and the correction log.
mod common;
use common::{config_reply, live_reply, FakeApi};
use eframe::egui::Vec2;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use neo_common::config::{Direction, HatMode, Mode};
use neo_config::diagrams::Hand;
use neo_config::launch::Signals;
use neo_config::map::{Focus, Side};
use neo_config::mapping::{Layer, Neo, Slot};
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

/// A profile whose shift control is S14, with a detent and the sidestick hat feeding targets.
fn shifted() -> FakeApi {
    let fake = FakeApi::idle();
    let mut config = neo_daemon::config::defaults();
    config.shift = Some("s14".into());
    config.buttons.insert("q9".into(), vec![5]);
    config.pov.insert("s16".into(), Direction::Up);
    fake.reply("/api/config", Ok(config_reply(json!({"config": config}))));
    fake
}

#[test]
fn select_build_record_undo_and_deselect() {
    let fake = FakeApi::idle();
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "Sidestick button 1");
    for target in ["NEO 2 (Missile)", "NEO 1 (Gun)", "NEO POV up", "Pulse"] {
        click(&mut harness, target);
    }
    let proposal = harness.state().state.map.proposal.clone().unwrap();
    assert_eq!((proposal.buttons, proposal.pov, proposal.mode), (vec![2], Some(Direction::Up), Mode::Pulse));
    harness.snapshot("mapping-changes-1600x900");
    click(&mut harness, "Record mapping");
    settle(&mut harness, |app| !app.state.map.busy);
    assert_eq!(harness.state().state.saved.as_ref().unwrap().buttons["s1"], [2]);
    assert_eq!(harness.state().state.map.note.as_deref(), Some("Recorded Sidestick button 1: NEO 2 (Missile), NEO POV up \u{b7} pulse."));
    assert_eq!(fake.bodies().iter().filter(|(path, _)| path == "/api/labels").count(), 1);
    // Undo changes goes back to the saved mapping; Deselect clears the selection.
    click(&mut harness, "NEO 3 (Weapon select)");
    click(&mut harness, "Undo changes");
    assert!(!harness.state().state.map.changed(harness.state().state.saved.as_ref().unwrap()));
    click(&mut harness, "Deselect");
    assert_eq!(harness.state().state.map.focus(), Focus::Nothing);
    // A refused save is reported where the actions are.
    fake.reply("PUT /api/config", Err(neo_config::api::ApiError::Rejected("Invalid profile.".into())));
    click(&mut harness, "Sidestick button 2");
    click(&mut harness, "NEO 4 (Target select)");
    click(&mut harness, "Record mapping");
    settle(&mut harness, |app| !app.state.map.busy);
    assert_eq!(harness.state().state.map.note.as_deref(), Some("Not saved: Invalid profile."));
}

#[test]
fn inspecting_a_target_shows_what_feeds_it() {
    let fake = shifted();
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "NEO 5 (High-G)");
    assert_eq!(harness.state().state.map.reverse, Some(Neo::Button(5)));
    harness.snapshot("mapping-inspect-1600x900");
    click(&mut harness, "NEO BUTTONS");
    assert_eq!(harness.state().state.map.side, Side::Buttons);
    assert!(harness.query_by_label("NEO 5: High-G, fed by S5 \u{b7} Q1 \u{b7} Q2 \u{b7} Q9").is_some());
    assert!(harness.query_by_label("NEO POV: Hat, 4 directions \u{b7} pick an arrow on the diagram, fed by U: S16").is_some());
    assert!(harness.query_by_label("NEO 16: On the hardware, not in the NEO report, fed by \u{2014}").is_some());
    click(&mut harness, "NEO 11: No AC8 action, fed by S11");
    assert_eq!(harness.state().state.map.reverse, Some(Neo::Button(11)));
    click(&mut harness, "NEO POV up");
    harness.snapshot("mapping-buttons-1600x900");
    // With a selection, rows toggle its targets.
    click(&mut harness, "Sidestick button 3");
    click(&mut harness, "NEO BUTTONS");
    click(&mut harness, "NEO 2: Missile, fed by S2");
    assert_eq!(harness.state().state.map.proposal.clone().unwrap().buttons, [2, 3]);
    click(&mut harness, "NEO POV left");
    harness.run_ok();
    click(&mut harness, "LOG \u{b7} 0");
    assert_eq!(harness.state().state.map.side, Side::Log);
}

#[test]
fn layers_actions_hand_and_hat() {
    let fake = shifted();
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "Shift layer");
    click(&mut harness, "Sidestick button 1");
    assert_eq!(harness.state().state.map.layer, Layer::Shift);
    harness.snapshot("mapping-shift-1600x900");
    click(&mut harness, "Long press");
    assert_eq!(harness.state().state.map.slot, Slot::Press);
    click(&mut harness, "Normal layer");
    click(&mut harness, "Long press");
    assert_eq!(harness.state().state.map.slot, Slot::Long);
    harness.snapshot("mapping-long-press-1600x900");
    click(&mut harness, "Double tap");
    click(&mut harness, "Press");
    // Choosing the option already chosen changes nothing.
    click(&mut harness, "Right-hand setup");
    assert_eq!(harness.state().state.map.hand, Hand::Right);
    click(&mut harness, "Left-hand setup");
    assert_eq!(harness.state().state.map.hand, Hand::Left);
    click(&mut harness, "Hat mapped");
    settle(&mut harness, |app| app.state.saved.as_ref().is_some_and(|saved| saved.hat == HatMode::Mapped));
    click(&mut harness, "Sidestick hat up");
    assert_eq!(harness.state().state.map.selected.as_deref(), Some("hat-up"));
    click(&mut harness, "Quadrant add-on input 20");
    click(&mut harness, "Quadrant detent state 9");
    assert_eq!(harness.state().state.map.selected.as_deref(), Some("q9"));
}

#[test]
fn live_presses_select_and_hold() {
    let fake = FakeApi::idle();
    let session = json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true});
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": session, "pressed": ["s2", "s3", "q20", "q9"]}))));
    let mut harness = open(&fake, WIDE, Duration::from_millis(5));
    settle(&mut harness, |app| app.state.map.selected.is_some());
    assert_eq!(harness.state().state.map.selected.as_deref(), Some("s2"));
    assert!(harness.query_by_label("Now sending: Sidestick button 2, Sidestick button 3, Quadrant add-on input 20, Quadrant detent state 9").is_some());
    harness.snapshot("mapping-live-1600x900");
}

#[test]
fn the_log_lists_corrections_newest_first() {
    let fake = FakeApi::idle();
    let entries = json!([
        {"label": "Sidestick button 1", "sources": ["s1"], "mapping": "NEO 1 (Gun) \u{2192} NEO 2 (Missile)"},
        {"label": "Quadrant button 7", "sources": ["q7"], "mapping": "Recorded by hand"}
    ]);
    fake.reply("/api/labels", Ok(json!({"labels": entries}).to_string()));
    let mut harness = open(&fake, NARROW, SLOW);
    // A target nothing feeds, and the table with no POV mappings.
    click(&mut harness, "NEO POV down");
    assert!(harness.query_by_label("Inspecting NEO POV down \u{b7} fed by nothing").is_some());
    click(&mut harness, "NEO BUTTONS");
    assert!(harness.query_by_label("NEO POV: Hat, 4 directions \u{b7} pick an arrow on the diagram, fed by \u{2014}").is_some());
    click(&mut harness, "LOG \u{b7} 2");
    harness.snapshot("mapping-log-1280x800");
    click(&mut harness, "Remove Quadrant button 7 from the correction log");
    settle(&mut harness, |app| app.state.map.labels.len() == 1);
    assert_eq!(harness.state().state.map.note.as_deref(), Some("Removed from the correction log. The mapping itself is unchanged."));
}

#[test]
fn hover_and_keyboard_focus_show_on_every_control() {
    let fake = shifted();
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "Sidestick button 1");
    click(&mut harness, "NEO 2 (Missile)");
    let labels = [
        "Right-hand setup", "Quadrant detent state 10", "Record mapping", "Undo changes", "NEO 3 (Weapon select)", "NEO 16: on the hardware, not in the NEO report",
        "Toggle", "Long press", "Shift layer", "Sidestick button 4", "NEO BUTTONS",
    ];
    for label in labels {
        let node = harness.get_all_by_label(label).next().unwrap_or_else(|| panic!("no {label}"));
        node.hover();
        harness.run_ok();
        harness.get_all_by_label(label).next().unwrap().focus();
        harness.run_ok();
    }
    click(&mut harness, "NEO BUTTONS");
    let row = "NEO 4: Target select, fed by S4";
    harness.get_by_label(row).hover();
    harness.run_ok();
    harness.get_by_label(row).focus();
    harness.run_ok();
    // The tab follows the keyboard focus (Tab moves it there).
    harness.get_by_label("LOG \u{b7} 0").focus();
    harness.run_ok();
    assert_eq!(harness.state().state.map.side, Side::Log);
    click(&mut harness, "LOG \u{b7} 0");
    // A profile without a shift control: the Shift option is disabled and says why.
    let plain = FakeApi::idle();
    let mut harness = open(&plain, NARROW, SLOW);
    harness.get_by_label("Shift layer").hover();
    harness.run_ok();
    assert_eq!(harness.state().state.map.layer, Layer::Normal);
}

#[test]
fn snapshot_idle_1280_by_800() {
    let fake = FakeApi::idle();
    let mut harness = open(&fake, NARROW, SLOW);
    harness.snapshot("mapping-idle-1280x800");
}
