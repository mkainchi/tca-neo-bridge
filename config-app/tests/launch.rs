//! Start-up without a window: the handshake, one copy at a time, the daemon's events and the exit codes.
mod common;
use common::TOKEN;
use eframe::egui;
use neo_config::ffi::{self, HANDLE, HWND, PCWSTR};
use neo_config::launch::{self, Handshake, Signals, SystemApi};
use neo_config::{App, Apis};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

#[derive(Default)]
struct Fake {
    mutex: HANDLE,
    error: u32,
    closed: Vec<HANDLE>,
    /// Event handles by name; a missing name fails to open.
    events: Vec<(String, HANDLE)>,
    opened: Vec<String>,
    waits: VecDeque<u32>,
    told: Vec<(String, String, u32)>,
    stdin: String,
    window: bool,
    built: bool,
}

static FAKE: LazyLock<Mutex<Fake>> = LazyLock::new(Default::default);
static SERIAL: Mutex<()> = Mutex::new(());

fn fake() -> MutexGuard<'static, Fake> {
    FAKE.lock().unwrap_or_else(|e| e.into_inner())
}

fn reset() -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    *fake() = Fake {
        mutex: 9,
        events: vec![(launch::SHOW_EVENT.into(), 61), (launch::QUIT_EVENT.into(), 62)],
        stdin: format!("4242 {TOKEN}\n"),
        window: true,
        ..Default::default()
    };
    guard
}

fn wide(text: PCWSTR) -> String {
    let mut length = 0;
    // SAFETY: test strings are NUL-terminated.
    unsafe {
        while *text.add(length) != 0 {
            length += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(text, length))
    }
}

unsafe extern "system" fn create_mutex(_: *const c_void, owner: ffi::BOOL, name: PCWSTR) -> HANDLE {
    assert_eq!((owner, wide(name).as_str()), (1, launch::MUTEX));
    fake().mutex
}
unsafe extern "system" fn last_error() -> u32 {
    fake().error
}
unsafe extern "system" fn open_event(access: u32, inherit: ffi::BOOL, name: PCWSTR) -> HANDLE {
    assert_eq!((access, inherit), (0x0010_0002, 0));
    let name = wide(name);
    let mut f = fake();
    f.opened.push(name.clone());
    f.events.iter().find(|(event, _)| *event == name).map_or(0, |(_, handle)| *handle)
}
unsafe extern "system" fn wait_many(count: u32, handles: *const HANDLE, all: ffi::BOOL, milliseconds: u32) -> u32 {
    assert_eq!((count, std::slice::from_raw_parts(handles, 2), all, milliseconds), (2, &[61, 62][..], 0, u32::MAX));
    fake().waits.pop_front().unwrap_or(u32::MAX)
}
unsafe extern "system" fn close(handle: HANDLE) -> ffi::BOOL {
    fake().closed.push(handle);
    1
}
unsafe extern "system" fn message_box(_: HWND, text: PCWSTR, caption: PCWSTR, kind: u32) -> i32 {
    fake().told.push((wide(text), wide(caption), kind));
    1
}

fn system() -> SystemApi {
    SystemApi { create_mutex, last_error, open_event, wait_many, close, message_box }
}

fn stdin() -> String {
    fake().stdin.clone()
}

/// Builds the app the way eframe would, on a fresh context, then "closes" the window at once.
fn window(options: eframe::NativeOptions, app: Box<dyn FnOnce(&egui::Context) -> App>) -> bool {
    assert_eq!(options.viewport.min_inner_size, Some(egui::vec2(1100.0, 700.0)));
    let shown = fake().window;
    if shown {
        let app = app(&egui::Context::default());
        fake().built = true;
        drop(app);
    }
    shown
}

fn apis() -> Apis {
    Apis { system: system(), stdin, window, fonts: std::env::temp_dir().join("no-fonts-here"), log: crash_log() }
}

/// One per test process: the panic hook keeps the path it was installed with.
fn crash_log() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("neo-config-crash-{}", std::process::id())).join(launch::CRASH_LOG)
}

#[test]
fn the_handshake_is_a_port_and_a_48_digit_token() {
    assert_eq!(launch::handshake(&format!("4242 {TOKEN}\r\n")), Some(Handshake { port: 4242, token: TOKEN.into() }));
    for bad in ["", "\n", "4242", &format!("0 {TOKEN}"), &format!("x {TOKEN}"), "4242 abc", &format!("4242 {}", TOKEN.to_uppercase()), &format!("4242 {}g", &TOKEN[1..])] {
        assert_eq!(launch::handshake(bad), None, "{bad:?}");
    }
}

#[test]
fn started_without_the_daemon_it_says_where_to_open_it() {
    let _g = reset();
    fake().stdin = String::new();
    assert_eq!(neo_config::run(apis()), 2);
    let f = fake();
    assert_eq!(f.told, [(neo_config::NOT_FROM_TRAY.to_string(), "TCA-NEO bridge".to_string(), 0x40)]);
    assert!(!f.built);
}

#[test]
fn a_second_copy_exits_and_a_failed_mutex_too() {
    let _g = reset();
    fake().error = ffi::ERROR_ALREADY_EXISTS;
    assert_eq!(neo_config::run(apis()), 3);
    fake().error = 0;
    fake().mutex = 0;
    assert_eq!(neo_config::run(apis()), 3);
    let f = fake();
    assert_eq!(f.closed, [9, 0]);
    assert!(!f.built);
}

#[test]
fn the_window_runs_with_the_daemons_events_and_its_failure_is_reported() {
    let _g = reset();
    assert_eq!(neo_config::run(apis()), 0);
    assert!(fake().built);
    assert_eq!(fake().opened, [launch::SHOW_EVENT, launch::QUIT_EVENT]);
    assert_eq!(fake().closed, [9]);
    // Without the events (no daemon made them) the window still opens; a window that cannot open is code 4.
    fake().events.clear();
    fake().built = false;
    assert_eq!(neo_config::run(apis()), 0);
    assert!(fake().built);
    fake().window = false;
    assert_eq!(neo_config::run(apis()), 4);
}

#[test]
fn events_open_together_or_not_at_all() {
    let _g = reset();
    assert_eq!(launch::events(&system()), Some([61, 62]));
    fake().events.remove(1);
    assert_eq!(launch::events(&system()), None);
    assert_eq!(fake().closed, [61]);
    fake().events.clear();
    assert_eq!(launch::events(&system()), None);
    assert_eq!(fake().closed, [61]);
}

#[test]
fn the_watcher_raises_show_then_quit_and_stops() {
    let _g = reset();
    fake().waits = [0, 0, 1].into();
    let signals = Arc::new(Signals::default());
    let woken = Arc::new(Mutex::new(Vec::new()));
    let seen = (signals.clone(), woken.clone());
    launch::watch(system(), [61, 62], signals.clone(), Box::new(move || seen.1.lock().unwrap().push((seen.0.show.load(Ordering::SeqCst), seen.0.quit.load(Ordering::SeqCst))))).join().unwrap();
    assert_eq!(*woken.lock().unwrap(), [(true, false), (true, false), (true, true)]);
    // A failed wait ends the watcher without a signal.
    let quiet = Arc::new(Signals::default());
    launch::watch(system(), [61, 62], quiet.clone(), Box::new(|| panic!("no signal expected"))).join().unwrap();
    assert!(!quiet.show.load(Ordering::SeqCst) && !quiet.quit.load(Ordering::SeqCst));
    let _ = (SystemApi::real(), Apis::real());
}

#[test]
fn an_internal_error_is_logged_and_reported() {
    let _g = reset();
    let _ = std::fs::remove_dir_all(crash_log().parent().unwrap());
    // Running installs the hook (once): a later panic anywhere is recorded and the user is told.
    fake().stdin = String::new();
    neo_config::run(apis());
    fake().told.clear();
    assert!(std::thread::spawn(|| panic!("boom at start")).join().is_err());
    assert_eq!(fake().told, [(launch::CRASHED.to_string(), "TCA-NEO bridge".to_string(), 0x40)]);
    let log = std::fs::read_to_string(crash_log()).unwrap();
    // One line: when, where it panicked, and the message.
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(log.contains("panicked at") && log.contains("launch.rs") && log.trim_end().ends_with("boom at start"), "{log}");
    // A log that cannot be written still leaves the message.
    fake().told.clear();
    launch::crashed(&system(), crash_log().parent().unwrap(), "x");
    assert_eq!(fake().told.len(), 1);
    std::fs::remove_dir_all(crash_log().parent().unwrap()).unwrap();
}