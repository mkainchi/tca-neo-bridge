//! The Mapping page's state: selecting, building, recording, the hat mode, the log and pressing controls.
mod common;
use common::config_reply;
use neo_common::config::{Config, Direction, HatMode, Mode};
use neo_common::labels::Entry;
use neo_config::map::{Focus, Map, Side};
use neo_config::mapping::{Layer, Mapping, Neo, Slot};
use neo_config::state::{Page, State};
use neo_config::worker::{Loaded, Reply, Request};
use serde_json::json;

fn defaults() -> Config {
    neo_daemon::config::defaults()
}

fn entry(label: &str) -> Entry {
    Entry { label: label.into(), sources: vec!["s1".into()], mapping: "NEO 1 (Gun) \u{2192} Nothing".into() }
}

#[test]
fn selecting_inspecting_and_building() {
    let saved = defaults();
    let mut map = Map::default();
    assert_eq!((map.focus(), map.context(&saved), map.changed(&saved)), (Focus::Nothing, "Select a TCA control to light up its NEO targets".into(), false));
    map.pick(Neo::Button(5));
    assert_eq!((map.focus(), map.context(&saved)), (Focus::Inspecting, "Inspecting NEO 5 (High-G) \u{b7} fed by S5 \u{b7} Q1 \u{b7} Q2".into()));
    map.pick(Neo::Button(5));
    assert_eq!(map.reverse, None);
    map.pick(Neo::Pov(Direction::Up));
    assert_eq!(map.context(&saved), "Inspecting NEO POV up \u{b7} fed by nothing");
    map.side = Side::Log;
    map.select(&saved, "s1");
    assert_eq!((map.focus(), map.reverse, map.side), (Focus::Selected, None, Side::Selected));
    map.pick(Neo::Button(2));
    map.pick(Neo::Pov(Direction::Up));
    map.set_mode(Mode::Pulse);
    assert!(map.changed(&saved));
    assert_eq!(map.context(&saved), "Mapping of Sidestick button 1 \u{2192} NEO 1 (Gun), NEO 2 (Missile), NEO POV up \u{b7} pulse");
    map.revert(&saved);
    assert!(!map.changed(&saved));
    map.deselect();
    map.set_mode(Mode::Toggle);
    assert_eq!((map.selected.clone(), map.proposal.clone(), map.changed(&saved)), (None, None, false));
}

#[test]
fn actions_and_layers_change_what_is_edited() {
    let mut saved = defaults();
    saved.shift = Some("s14".into());
    saved.long_press.insert("s1".into(), neo_common::config::Target { buttons: vec![9], pov: None });
    let mut map = Map::default();
    map.select(&saved, "s1");
    map.set_slot(&saved, Slot::Long);
    assert_eq!(map.proposal.clone().unwrap().buttons, [9]);
    assert_eq!(map.subject("s1"), "Sidestick button 1 \u{b7} long press");
    map.set_layer(&saved, Layer::Shift);
    assert_eq!((map.slot, map.subject("s1")), (Slot::Press, "Sidestick button 1 (shift layer)".into()));
    let empty = map.proposal.clone().unwrap();
    assert_eq!(map.shown(&saved, &empty, Slot::Press), "Normal: NEO 1 (Gun)");
    assert_eq!(map.shown(&saved, &empty, Slot::Long), "Nothing");
    // A shift mapping of its own shows as itself.
    let own = Mapping { buttons: vec![3], pov: None, mode: Mode::Hold };
    assert_eq!(map.shown(&saved, &own, Slot::Press), "NEO 3 (Weapon select)");
    let pov = Mapping { buttons: vec![], pov: Some(Direction::Up), mode: Mode::Hold };
    assert_eq!(map.shown(&saved, &pov, Slot::Press), "NEO POV up");
    map.set_layer(&saved, Layer::Normal);
    assert_eq!(map.shown(&saved, &map.proposal.clone().unwrap(), Slot::Press), "NEO 1 (Gun)");
    // No shift control: the shift layer falls back to normal.
    map.set_layer(&saved, Layer::Shift);
    map.normalize(&saved);
    assert_eq!(map.layer, Layer::Shift);
    saved.shift = None;
    map.normalize(&saved);
    assert_eq!(map.layer, Layer::Normal);
    map.normalize(&saved);
    assert_eq!(map.layer, Layer::Normal);
}

#[test]
fn recording_saves_the_profile_and_logs_the_correction() {
    let saved = defaults();
    let mut map = Map::default();
    map.labels = vec![entry("Earlier")];
    assert_eq!(map.record(&saved), None);
    map.select(&saved, "s1");
    assert_eq!(map.record(&saved), None);
    map.pick(Neo::Button(1));
    map.pick(Neo::Button(2));
    let Some(Request::Record { config, labels }) = map.record(&saved) else { panic!("no request") };
    assert_eq!((config.buttons["s1"].clone(), labels.len()), (vec![2], 2));
    assert_eq!(labels[1], Entry { label: "Sidestick button 1".into(), sources: vec!["s1".into()], mapping: "NEO 1 (Gun) \u{2192} NEO 2 (Missile)".into() });
    assert!(map.busy);
    assert_eq!(map.record(&saved), None);
    map.recorded(Ok(labels), true);
    assert_eq!(map.note.as_deref(), Some("Recorded Sidestick button 1: NEO 2 (Missile). Applied to the running emulation."));
    assert_eq!((map.busy, map.labels.len()), (false, 2));
    map.pick(Neo::Button(3));
    map.record(&saved);
    map.recorded(Err("Disk full.".into()), false);
    assert_eq!(map.note.as_deref(), Some("Mapping saved, but the correction log was not updated: Disk full.."));
    map.record(&saved);
    map.not_saved("Invalid profile.");
    assert_eq!((map.busy, map.note.as_deref()), (false, Some("Not saved: Invalid profile.")));
    // Long mappings are cut to the log's 200 characters.
    for n in 1..=15 {
        map.pick(Neo::Button(n));
    }
    map.pick(Neo::Pov(Direction::Left));
    map.set_mode(Mode::PulseChange);
    let Some(Request::Record { labels, .. }) = map.record(&saved) else { panic!("no request") };
    assert_eq!(labels.last().unwrap().mapping.chars().count(), 200);
}

#[test]
fn the_hat_mode_saves_at_once() {
    let saved = defaults();
    let mut map = Map::default();
    assert_eq!(map.set_hat(&saved, HatMode::Camera), None);
    let Some(Request::SetHat(config)) = map.set_hat(&saved, HatMode::Mapped) else { panic!("no request") };
    assert_eq!(config.hat, HatMode::Mapped);
    assert_eq!(map.set_hat(&saved, HatMode::Mapped), None);
    map.hat_set(HatMode::Mapped, false);
    assert_eq!(map.note.as_deref(), Some("The hat now sends its mapped controls; the camera mini-stick gets no input."));
    map.select(&config, "hat-up");
    map.hat_set(HatMode::Camera, true);
    assert_eq!(map.selected, None);
    assert_eq!(map.note.as_deref(), Some("The hat drives the camera again; hat mappings are kept but inactive. Applied to the running emulation."));
    map.select(&saved, "s2");
    map.hat_set(HatMode::Camera, false);
    assert_eq!(map.selected.as_deref(), Some("s2"));
}

#[test]
fn the_log_removes_entries_but_keeps_mappings() {
    let mut map = Map::default();
    map.labels = vec![entry("A"), entry("B"), entry("C")];
    let Request::Forget(labels) = map.forget(1) else { panic!("not a removal") };
    assert_eq!(labels.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), ["A", "C"]);
    map.forgot(Ok(labels));
    assert_eq!((map.labels.len(), map.note.as_deref()), (2, Some("Removed from the correction log. The mapping itself is unchanged.")));
    map.forgot(Err("Locked.".into()));
    assert_eq!((map.labels.len(), map.note.as_deref()), (2, Some("Not removed: Locked.")));
}

#[test]
fn pressing_a_control_selects_it_while_the_page_is_open() {
    let mut saved = defaults();
    let mut map = Map::default();
    assert_eq!(map.sending(), "Start emulation to see presses.");
    map.emulating = true;
    assert_eq!(map.sending(), "Nothing pressed");
    let pressed = |keys: &[&str]| keys.iter().map(|key| key.to_string()).collect::<Vec<_>>();
    map.press(Some(&saved), &pressed(&["hat-up", "s3"]), true);
    assert_eq!((map.selected.as_deref(), map.sending()), (Some("s3"), "Now sending: Sidestick hat up, Sidestick button 3".into()));
    // Still held: no new selection.
    map.deselect();
    map.press(Some(&saved), &pressed(&["hat-up", "s3"]), true);
    assert_eq!(map.selected, None);
    // Off the page, while saving, or before the profile loads: presses only show.
    map.press(Some(&saved), &pressed(&["s4"]), false);
    map.busy = true;
    map.press(Some(&saved), &pressed(&["s5"]), true);
    map.busy = false;
    map.press(None, &pressed(&["s6"]), true);
    assert_eq!((map.selected.clone(), map.held.clone()), (None, pressed(&["s6"])));
    // The hat selects only while mapped; unknown sources never do.
    saved.hat = HatMode::Mapped;
    map.press(Some(&saved), &pressed(&["s6", "x1", "hat-down"]), true);
    assert_eq!(map.selected.as_deref(), Some("hat-down"));
    assert!(!Map::selectable(&saved, "x1") && Map::selectable(&saved, "q31"));
}

#[test]
fn the_window_state_applies_mapping_replies() {
    let mut state = State::default();
    let config = serde_json::from_str(&config_reply(json!({"links": {"source": "tca", "hidden": true, "tca": true, "virtual_neo": true}}))).unwrap();
    state.apply(Reply::Loaded(Box::new(Loaded { config, profiles: Err("Locked.".into()), labels: Err("Unreadable.".into()) })));
    assert_eq!(state.note.as_deref(), Some("Saved profiles could not be listed: Locked."));
    assert_eq!(state.map.note.as_deref(), Some("Earlier corrections could not be loaded: Unreadable."));
    assert!(state.map.emulating && state.profiles.profiles.is_empty());
    let mut next = defaults();
    next.buttons.insert("q20".into(), vec![3]);
    state.apply(Reply::Recorded { config: Box::new(next.clone()), running: false, log: Ok(vec![entry("A")]) });
    assert_eq!((state.saved.clone(), state.map.labels.len()), (Some(next.clone()), 1));
    next.hat = HatMode::Mapped;
    state.apply(Reply::HatSet { config: Box::new(next.clone()), running: false });
    assert_eq!(state.saved.as_ref().map(|c| c.hat), Some(HatMode::Mapped));
    state.apply(Reply::NotSaved("No.".into()));
    assert_eq!(state.map.note.as_deref(), Some("Not saved: No."));
    state.apply(Reply::Forgot(Ok(vec![])));
    assert!(state.map.labels.is_empty());
    // Live presses select only on the Mapping page.
    let live = |pressed: &[&str]| Reply::Live(Box::new(serde_json::from_value(json!({"pressed": pressed, "levers": null, "monitor": null, "paused": false, "preview": false, "warnings": [], "status": "Emulating", "links": null})).unwrap()));
    state.page = Page::Live;
    state.apply(live(&["s9"]));
    assert_eq!(state.map.selected, None);
    state.page = Page::Mapping;
    state.apply(live(&["s9", "s10"]));
    assert_eq!((state.map.selected.as_deref(), state.map.emulating), (Some("s10"), false));
}
