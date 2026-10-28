//! The Live page and the profile menu through egui_kittest against a scripted API, with fake file dialogs.
mod common;
use common::{live_reply, FakeApi};
use eframe::egui::{Key, Role, Vec2};
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

/// A session with the stick deflected, NEO 1 and 9 held and the POV up.
fn session(fake: &FakeApi) {
    let links = json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true});
    let monitor = json!({"roll": 0.42, "pitch": -0.42, "yaw": 0.08, "slider": 0.5, "levers": [0.66, 0.2], "axes": [46000, 19000, 22000, 32768, 0, 35000, 32768], "armed": true, "precision": false, "shift": false, "buttons": 0b1_0000_0001, "pov": 0, "head": null});
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "links": links, "monitor": monitor, "levers": [0.66, 0.2], "pressed": ["s1", "hat-up"]}))));
}

fn snapshot_live(size: Vec2, name: &str) {
    let fake = FakeApi::idle();
    session(&fake);
    let mut harness = open(&fake, size, Duration::from_millis(20));
    settle(&mut harness, |app| app.state.live());
    click(&mut harness, "Live");
    harness.snapshot(name);
}

#[test]
fn snapshot_live_1600_by_900() {
    snapshot_live(WIDE, "live-1600x900");
}

#[test]
fn snapshot_live_1280_by_800() {
    snapshot_live(NARROW, "live-1280x800");
}

#[test]
fn snapshot_profile_menu() {
    let fake = FakeApi::idle();
    fake.reply("/api/profiles", Ok(json!({"profiles": ["Cruise", "Dogfight"], "active": "Dogfight"}).to_string()));
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "Profile: Dogfight");
    harness.snapshot("profile-menu-1600x900");
}

/// What the fake dialogs return and saw, per test thread (the harness runs the app on the test's own thread).
mod dialogs {
    use neo_config::ffi::{BOOL, HANDLE, HWND, OPENFILENAMEW, PCWSTR};
    use std::cell::RefCell;

    thread_local! {
        /// The path the next dialog returns; None cancels it.
        pub static CHOSEN: RefCell<Option<String>> = const { RefCell::new(None) };
        /// (flags, suggested file name) of every dialog shown.
        pub static SHOWN: RefCell<Vec<(u32, String)>> = const { RefCell::new(Vec::new()) };
        pub static OPENED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn read(text: PCWSTR) -> String {
        // SAFETY: the app passes NUL-terminated strings.
        let length = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) })
    }

    pub unsafe extern "system" fn dialog(dialog: *mut OPENFILENAMEW) -> BOOL {
        // SAFETY: the app passes a filled-in structure whose file buffer holds nMaxFile units.
        let dialog = unsafe { &mut *dialog };
        let file = unsafe { std::slice::from_raw_parts_mut(dialog.lpstrFile, dialog.nMaxFile as usize) };
        SHOWN.with_borrow_mut(|shown| shown.push((dialog.Flags, read(file.as_ptr()))));
        let Some(path) = CHOSEN.take() else { return 0 };
        let units: Vec<u16> = path.encode_utf16().collect();
        file[..units.len()].copy_from_slice(&units);
        if units.len() < file.len() {
            file[units.len()] = 0;
        }
        1
    }

    pub unsafe extern "system" fn shell(_: HWND, verb: PCWSTR, file: PCWSTR, _: PCWSTR, _: PCWSTR, show: i32) -> HANDLE {
        assert_eq!((read(verb).as_str(), show), ("open", 1));
        OPENED.with_borrow_mut(|opened| opened.push(read(file)));
        42
    }

    pub fn api() -> neo_config::files::FileApi {
        neo_config::files::FileApi { open: dialog, save: dialog, shell }
    }
}

#[test]
fn dialogs_suggest_a_name_and_return_the_chosen_path() {
    use neo_config::files::{choose_export, choose_import, open};
    let api = dialogs::api();
    assert_eq!(choose_import(&api), None);
    dialogs::CHOSEN.set(Some(r"C:\in\Cruise.json".into()));
    assert_eq!(choose_import(&api), Some(PathBuf::from(r"C:\in\Cruise.json")));
    dialogs::CHOSEN.set(Some(r"C:\out\Dogfight.json".into()));
    assert_eq!(choose_export(&api, "Dogfight.json"), Some(PathBuf::from(r"C:\out\Dogfight.json")));
    // A name as long as the buffer is cut to fit, and a path that fills it is read whole.
    let long = "x".repeat(2000);
    dialogs::CHOSEN.set(Some("y".repeat(1024)));
    assert_eq!(choose_export(&api, &long), Some(PathBuf::from("y".repeat(1024))));
    let shown = dialogs::SHOWN.take();
    assert_eq!(shown.iter().map(|(flags, name)| (*flags & 0x1002, name.len())).collect::<Vec<_>>(), [(0x1000, 0), (0x1000, 0), (0x0002, 13), (0x0002, 1023)]);
    open(&api, std::path::Path::new(r"C:\t\sheet.html"));
    assert_eq!(dialogs::OPENED.take(), [r"C:\t\sheet.html"]);
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-config-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_with_files(fake: &FakeApi, temp: PathBuf) -> Harness<'static, App> {
    let api = fake.clone();
    let mut harness = Harness::builder()
        .with_size(WIDE)
        .build_eframe(move |cc| App::new(&cc.egui_ctx, Box::new(api), Arc::new(Signals::default()), SLOW, &fonts()).with_files(dialogs::api(), temp));
    settle(&mut harness, |app| app.state.saved.is_some());
    harness
}

fn note(harness: &Harness<'static, App>) -> Option<(String, bool)> {
    harness.state().state.menu.note.clone()
}

#[test]
fn import_export_and_print_go_through_the_dialogs() {
    let dir = temp("files");
    let fake = FakeApi::idle();
    let mut harness = open_with_files(&fake, dir.clone());
    click(&mut harness, "Profile: Unsaved profile");
    harness.get_by_label("No saved profiles yet. Save the current one under a name.");
    // Cancelled dialogs change nothing.
    click(&mut harness, "Import a profile file");
    click(&mut harness, "Export the saved settings");
    assert_eq!(note(&harness), None);
    // Import: the file goes to the daemon, which answers with the settings it saved.
    let file = dir.join("Old.json");
    std::fs::write(&file, format!("{}\n", neo_common::config::to_json(&neo_daemon::config::defaults()))).unwrap();
    dialogs::CHOSEN.set(Some(file.to_string_lossy().into_owned()));
    click(&mut harness, "Import a profile file");
    settle(&mut harness, |app| app.state.menu.note.is_some());
    assert_eq!(note(&harness), Some(("Imported Old.json. Save it under a name to keep it as a profile.".into(), false)));
    assert!(fake.bodies().iter().any(|(path, body)| path == "/api/config" && body.contains("\"version\"")));
    dialogs::CHOSEN.set(Some(dir.join("Missing.json").to_string_lossy().into_owned()));
    click(&mut harness, "Import a profile file");
    assert_eq!(note(&harness), Some(("Not imported: Missing.json could not be read.".into(), true)));
    // Export suggests the bridge's name when no profile is active.
    let out = dir.join("Out.json");
    dialogs::CHOSEN.set(Some(out.to_string_lossy().into_owned()));
    click(&mut harness, "Export the saved settings");
    assert_eq!(note(&harness), Some(("Exported Out.json.".into(), false)));
    assert!(out.exists());
    assert_eq!(dialogs::SHOWN.take().last().unwrap().1, "tca-neo-bridge.json");
    // Print writes the sheet and opens it in the browser.
    click(&mut harness, "Print cheat sheet");
    let sheet = dir.join("tca-neo-bridge-cheat-sheet.html");
    assert_eq!(dialogs::OPENED.take(), [sheet.to_string_lossy().into_owned()]);
    assert!(std::fs::read_to_string(&sheet).unwrap().contains("<p>Profile: Unsaved profile</p>"));
    // Nothing to print before the profile loads.
    harness.state_mut().state.saved = None;
    harness.state_mut().state.menu.want = Some(neo_config::state::Want::Print);
    harness.run_ok();
    assert!(dialogs::OPENED.take().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_cheat_sheet_that_cannot_be_written_is_reported() {
    let fake = FakeApi::idle();
    let mut harness = open_with_files(&fake, std::env::temp_dir().join("neo-config-missing").join("nested"));
    click(&mut harness, "Print cheat sheet");
    assert_eq!(note(&harness), Some(("Not printed: the cheat sheet could not be written.".into(), true)));
    assert!(dialogs::OPENED.take().is_empty());
}

fn action(profiles: &[&str], active: Option<&str>) -> Result<String, neo_config::api::ApiError> {
    Ok(json!({"profiles": profiles, "active": active, "config": neo_daemon::config::defaults(), "running": false}).to_string())
}

#[test]
fn the_profile_menu_switches_saves_and_deletes() {
    let fake = FakeApi::idle();
    fake.reply("/api/profiles", Ok(json!({"profiles": ["Cruise", "Dogfight"], "active": "Dogfight"}).to_string()));
    fake.reply("POST /api/profiles/Cruise/activate", action(&["Cruise", "Dogfight"], Some("Cruise")));
    fake.reply("PUT /api/profiles/Attack", action(&["Attack", "Cruise", "Dogfight"], Some("Attack")));
    fake.reply("DELETE /api/profiles/Attack", action(&["Cruise", "Dogfight"], None));
    let mut harness = open(&fake, WIDE, SLOW);
    harness.get_by_label("Profile: Dogfight").hover();
    harness.run_ok();
    click(&mut harness, "Profile: Dogfight");
    for label in ["Cruise", "Profile: Dogfight"] {
        harness.get_by_label(label).hover();
        harness.run_ok();
        harness.get_by_label(label).focus();
        harness.run_ok();
    }
    click(&mut harness, "Cruise");
    settle(&mut harness, |app| app.state.profile_name() == "Cruise");
    assert_eq!(note(&harness), Some(("Switched to Cruise.".into(), false)));
    // An empty name is refused; Enter in the field saves.
    click(&mut harness, "Save as");
    assert!(harness.state().state.menu.invalid);
    harness.get_by_role_and_label(Role::TextInput, "New profile name").focus();
    harness.run_ok();
    harness.get_by_role_and_label(Role::TextInput, "New profile name").type_text("Attack");
    harness.run_ok();
    harness.key_press(Key::Enter);
    settle(&mut harness, |app| app.state.profile_name() == "Attack");
    assert_eq!((note(&harness), harness.state().state.menu.name.as_str()), (Some(("Saved as Attack.".into(), false)), ""));
    // Delete asks first.
    click(&mut harness, "Delete");
    click(&mut harness, "Delete Attack? Click again");
    settle(&mut harness, |app| app.state.profiles.active.is_none());
    assert_eq!(note(&harness), Some(("Deleted Attack. The current settings are kept.".into(), false)));
    // A failure is shown in the menu.
    click(&mut harness, "Dogfight");
    settle(&mut harness, |app| app.state.menu.note.as_ref().is_some_and(|(_, error)| *error));
    harness.snapshot("profile-menu-error-1600x900");
    // Escape and a click outside close the menu; the picker toggles it.
    harness.key_press(Key::Escape);
    harness.run_ok();
    assert!(!harness.state().state.menu.open);
    click(&mut harness, "Profile: Unsaved profile");
    click(&mut harness, "Profile: Unsaved profile");
    assert!(!harness.state().state.menu.open);
    click(&mut harness, "Profile: Unsaved profile");
    click(&mut harness, "Live");
    assert!(!harness.state().state.menu.open);
}

#[test]
fn the_live_page_waits_for_emulation() {
    let fake = FakeApi::idle();
    let mut harness = open(&fake, WIDE, SLOW);
    click(&mut harness, "Live");
    harness.get_by_label("Start emulation to see the TCA and what the NEO receives.");
    harness.get_by_label("Nothing pressed");
    assert_eq!(harness.get_all_by_label("POV centred").count(), 2);
    harness.snapshot("live-idle-1600x900");
}
#[test]
fn paused_input_shows_as_a_warning() {
    let fake = FakeApi::idle();
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "paused": true, "monitor": common::monitor()}))));
    let mut harness = open(&fake, WIDE, Duration::from_millis(20));
    settle(&mut harness, |app| app.state.paused);
    click(&mut harness, "Live");
    assert!(harness.get_all_by_label("Input paused").count() > 0);
}