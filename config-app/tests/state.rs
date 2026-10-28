//! The window's model: status pills, replies, warnings and the profile name.
mod common;
use common::{config_reply, live_reply};
use neo_common::api::{Links, Profiles};
use neo_config::state::{pills, Page, State, Tone};
use neo_config::worker::{Fatal, Loaded, Reply};
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
    state.apply(Reply::Failed("Busy.".into()), 0.0);
    assert_eq!(state.note.as_deref(), Some("Busy."));
    let config = serde_json::from_str(&config_reply(json!({"status": "Emulating", "preview": neo_daemon::config::defaults(), "warnings": ["Steam is running."]}))).unwrap();
    state.apply(Reply::Loaded(Box::new(Loaded { config, profiles: Ok(Profiles { profiles: vec!["Attack".into()], active: Some("Attack".into()) }), labels: Ok(vec![]) })), 0.0);
    assert_eq!((state.status.as_str(), state.previewing(), state.profile_name(), state.note.clone()), ("Emulating", true, "Attack", None));
    assert_eq!((state.saved.clone(), state.profile_path.as_str()), (Some(neo_daemon::config::defaults()), r"C:\data\config.json"));
    assert!(state.show_warnings());
    state.dismissed = state.warnings.clone();
    assert!(!state.show_warnings());
    let live = serde_json::from_str(&live_reply(json!({"status": "Emulating", "paused": true, "monitor": common::monitor(), "warnings": ["Steam is running.", "Steam Input is on."]}))).unwrap();
    state.apply(Reply::Live(Box::new(live)), 0.0);
    assert!(state.paused && state.live() && !state.previewing() && state.show_warnings());
    state.apply(Reply::Live(Box::new(serde_json::from_str(&live_reply(json!({})).to_string()).unwrap())), 0.0);
    assert!(!state.show_warnings() && !state.live());
    state.apply(Reply::Fatal(Fatal::Lost), 0.0);
    assert!(state.closing);
}

#[test]
fn the_settings_draft_follows_the_daemon() {
    use neo_config::settings::Field;
    use neo_config::worker::Request;
    let mut state = State::default();
    // A preview already running is the draft to keep editing.
    let mut preview = neo_daemon::config::defaults();
    preview.axes.roll.curve = 0.3;
    let config = serde_json::from_str(&config_reply(json!({"preview": preview}))).unwrap();
    state.apply(Reply::Loaded(Box::new(Loaded { config, profiles: Ok(Profiles { profiles: vec![], active: None }), labels: Ok(vec![]) })), 0.0);
    assert!(state.previewing());
    assert_eq!((state.settings.fields[&Field::Curve].as_str(), state.saved.as_ref().unwrap().axes.roll.curve), ("30", 0.0));
    // A queued preview goes out once the one in flight is answered.
    state.settings.sending = true;
    state.settings.queued = true;
    assert!(matches!(state.apply(Reply::Previewed, 0.0).as_slice(), [Request::Preview(_)]));
    state.apply(Reply::Previewed, 0.0);
    // Emulation stopped, so the daemon dropped the preview.
    state.apply(Reply::Live(Box::new(serde_json::from_str(&live_reply(json!({}))).unwrap())), 50.0);
    assert!(!state.previewing() && state.now == 50.0);
    // Saving makes the draft the saved profile.
    let mut saved = neo_daemon::config::defaults();
    saved.axes.roll.curve = 0.3;
    state.apply(Reply::Saved { config: Box::new(saved.clone()), running: true }, 0.0);
    assert_eq!((state.saved.clone(), state.settings.dirty), (Some(saved.clone()), false));
    for reply in [Reply::SaveFailed("No.".into()), Reply::PreviewFailed("No.".into()), Reply::PreviewEndFailed("No.".into()), Reply::PreviewEnded] {
        state.apply(reply, 0.0);
    }
    assert_eq!(state.settings.message.as_ref().map(|(text, _)| text.as_str()), Some("Preview not ended: No."));
    // Recorded mappings and the hat mode reach the draft too.
    let mut recorded = saved.clone();
    recorded.buttons.insert("q20".into(), vec![3]);
    recorded.behaviours.insert("q20".into(), neo_common::config::Mode::Toggle);
    state.apply(Reply::Recorded { config: Box::new(recorded), running: false, log: Ok(vec![]) }, 0.0);
    let draft = state.settings.draft.clone().unwrap();
    assert_eq!((draft.buttons["q20"].clone(), draft.axes.roll.curve), (vec![3], 0.3));
    let mut hat = saved;
    hat.hat = neo_common::config::HatMode::Mapped;
    state.apply(Reply::HatSet { config: Box::new(hat), running: false }, 0.0);
    assert_eq!(state.settings.draft.as_ref().unwrap().hat, neo_common::config::HatMode::Mapped);
    // Leaving the settings pages ends a preview; moving between them does not.
    state.settings.previewing = true;
    assert!(state.show(Page::Throttle).is_empty());
    assert_eq!(state.show(Page::Mapping), [Request::EndPreview]);
    assert!(Page::ALL.iter().filter(|page| page.settings()).count() == 4);
    // Before the profile loads there is no draft to update.
    let mut fresh = State::default();
    fresh.apply(Reply::Recorded { config: Box::new(neo_daemon::config::defaults()), running: false, log: Ok(vec![]) }, 0.0);
    fresh.apply(Reply::HatSet { config: Box::new(neo_daemon::config::defaults()), running: false }, 0.0);
    assert!(fresh.settings.draft.is_none());
}
#[test]
fn profile_actions_wait_for_unsaved_settings_and_valid_names() {
    use neo_config::worker::Request;
    let mut state = State::default();
    let defaults = neo_daemon::config::defaults();
    state.apply(Reply::Adopted { config: Box::new(defaults.clone()), profiles: None, text: "Imported a.json.".into() }, 0.0);
    assert_eq!((state.saved.clone(), state.menu.note.clone(), state.settings.dirty), (Some(defaults.clone()), Some(("Imported a.json.".into(), false)), false));
    assert_eq!(state.export_name(), "tca-neo-bridge.json");
    // No active profile: nothing to delete.
    assert!(state.delete().is_empty() && !state.menu.confirm_delete);
    let listed = Profiles { profiles: vec!["Cruise".into()], active: Some("Cruise".into()) };
    state.menu.name = "x".into();
    state.apply(Reply::Listed { profiles: listed.clone(), text: "Saved as Cruise.".into() }, 0.0);
    assert_eq!((state.profiles.clone(), state.menu.name.as_str(), state.export_name()), (listed.clone(), "", "Cruise.json".to_string()));
    state.apply(Reply::Adopted { config: Box::new(defaults.clone()), profiles: Some(Profiles { profiles: vec![], active: None }), text: "Switched.".into() }, 0.0);
    assert!(state.profiles.profiles.is_empty());
    state.apply(Reply::ProfileFailed("Not saved: Busy.".into()), 0.0);
    assert_eq!(state.menu.note, Some(("Not saved: Busy.".into(), true)));
    // Names follow the daemon's rule.
    state.menu.name = "  ".into();
    assert!(state.save_as().is_empty() && state.menu.invalid);
    assert_eq!(state.menu.note.as_ref().unwrap().0, "Use 1 to 40 letters, digits, spaces, _ or -.");
    state.menu.name = " Dog fight ".into();
    assert_eq!(state.save_as(), [Request::SaveAs("Dog fight".into())]);
    assert!(!state.menu.invalid);
    assert_eq!(state.activate("Cruise"), [Request::Activate("Cruise".into())]);
    assert!(matches!(state.import("{\"version\": 1}", "a.json").as_slice(), [Request::Import { name, .. }] if name == "a.json"));
    assert!(state.import("{", "b.json").is_empty());
    assert!(state.menu.note.as_ref().unwrap().0.starts_with("Not imported: Invalid profile: EOF"));
    // Unsaved edits block switching, saving under a name and importing.
    state.settings.dirty = true;
    assert!(state.activate("Cruise").is_empty() && state.save_as().is_empty() && state.import("{}", "a.json").is_empty());
    assert_eq!(state.menu.note, Some(("Save or discard unsaved settings first.".into(), true)));
    // Delete asks first; a second click deletes.
    state.profiles = listed;
    assert!(state.delete().is_empty() && state.menu.confirm_delete);
    assert_eq!(state.delete(), [Request::DeleteProfile("Cruise".into())]);
    assert!(!state.menu.confirm_delete);
}

#[test]
fn export_writes_the_saved_settings() {
    let dir = std::env::temp_dir().join(format!("neo-config-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut state = State::default();
    let path = dir.join("Cruise.json");
    // Nothing loaded yet: nothing to export.
    state.export(&path);
    assert!(!path.exists() && state.menu.note.is_none());
    state.saved = Some(neo_daemon::config::defaults());
    state.export(&path);
    assert_eq!(state.menu.note, Some(("Exported Cruise.json.".into(), false)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("{}\n", neo_common::config::to_json(&neo_daemon::config::defaults())));
    state.export(&dir);
    assert!(state.menu.note.as_ref().unwrap().1);
    state.export(std::path::Path::new(""));
    assert_eq!(state.menu.note, Some(("Not exported:  could not be written.".into(), true)));
    std::fs::remove_dir_all(dir).unwrap();
}