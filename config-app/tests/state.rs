//! The window's model: status pills, replies, warnings and the profile name.
mod common;
use common::{config_reply, live_reply};
use neo_common::api::{Links, Profiles};
use neo_config::state::{pills, Page, State, Tone};
use neo_config::worker::{Fatal, Reply};
use serde_json::json;

fn links(source: &str, hidden: bool, tca: bool, virtual_neo: bool) -> Links {
    Links { source: source.into(), hidden, tca, virtual_neo }
}

#[test]
fn pills_follow_the_status_and_the_session() {
    let tone = |status: &str| pills(status, None).status;
    assert_eq!(
        ["Emulating", "Starting", "Stopping", "Error: x", "unreachable", "unavailable (x)", "Idle", "Loading\u{2026}"].map(tone),
        [Tone::Ok, Tone::Busy, Tone::Busy, Tone::Fault, Tone::Fault, Tone::Fault, Tone::Off, Tone::Off]
    );
    let idle = pills("Idle", None);
    assert_eq!((idle.tca, idle.hidhide, idle.neo), ((Tone::Off, "TCA: not open"), (Tone::Off, "HidHide: off"), (Tone::Off, "Virtual NEO: absent")));
    let full = pills("Emulating", Some(&links("tca", true, true, true)));
    assert_eq!((full.tca, full.hidhide, full.neo), ((Tone::Ok, "TCA: sidestick + quadrant open"), (Tone::Ok, "HidHide: TCA hidden"), (Tone::Ok, "Virtual NEO present")));
    let reconnecting = pills("Emulating", Some(&links("tca", false, false, false)));
    assert_eq!((reconnecting.tca, reconnecting.hidhide), ((Tone::Busy, "TCA: reconnecting\u{2026}"), (Tone::Off, "HidHide: off")));
    let validation = pills("Emulating", Some(&links("validation", false, false, true)));
    assert_eq!((validation.tca, validation.hidhide), ((Tone::Busy, "Source: validation stream"), (Tone::Off, "HidHide: not needed")));
}

#[test]
fn pages_have_titles_and_help_in_rail_order() {
    assert_eq!(Page::ALL.map(Page::title), ["Mapping", "Flight axes", "Throttle", "Camera & head", "Buttons & helpers", "Live"]);
    assert!(Page::ALL.iter().all(|page| page.help().ends_with('.')));
}

#[test]
fn replies_update_the_model() {
    let mut state = State::default();
    assert_eq!((state.page, state.status.as_str(), state.profile_name(), state.live()), (Page::Mapping, "Loading\u{2026}", "Unsaved profile", false));
    state.apply(Reply::Failed("Busy.".into()));
    assert_eq!(state.note.as_deref(), Some("Busy."));
    let config = serde_json::from_str(&config_reply(json!({"status": "Emulating", "preview": neo_daemon::config::defaults(), "warnings": ["Steam is running."]}))).unwrap();
    state.apply(Reply::Loaded(Box::new(config), Profiles { profiles: vec!["Attack".into()], active: Some("Attack".into()) }));
    assert_eq!((state.status.as_str(), state.previewing, state.profile_name(), state.note.clone()), ("Emulating", true, "Attack", None));
    assert_eq!((state.saved.clone(), state.profile_path.as_str()), (Some(neo_daemon::config::defaults()), r"C:\data\config.json"));
    assert!(state.show_warnings());
    state.dismissed = state.warnings.clone();
    assert!(!state.show_warnings());
    let live = serde_json::from_str(&live_reply(json!({"status": "Emulating", "paused": true, "monitor": common::monitor(), "warnings": ["Steam is running.", "Steam Input is on."]}))).unwrap();
    state.apply(Reply::Live(Box::new(live)));
    assert!(state.paused && state.live() && !state.previewing && state.show_warnings());
    state.apply(Reply::Live(Box::new(serde_json::from_str(&live_reply(json!({})).to_string()).unwrap())));
    assert!(!state.show_warnings() && !state.live());
    state.apply(Reply::Fatal(Fatal::Lost));
    assert!(state.closing);
}
