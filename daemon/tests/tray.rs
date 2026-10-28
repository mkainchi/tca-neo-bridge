mod common;
use common::*;
use neo_daemon::app::{self, Apis, Options, SystemApi};
use neo_daemon::config_app::{self, ConfigApp, ConfigAppApi, Launched};
use neo_daemon::emulation::Source;
use neo_daemon::ffi::{self, HANDLE, HWND, MSG, NOTIFYICONDATAW, PCWSTR, POINT, WNDCLASSEXW};
use neo_daemon::shared::Status;
use neo_daemon::tray::{self, UiApi, ID_CONFIGURE, ID_QUIT, ID_START, ID_STOP, WM_COMMAND, WM_STATUS, WM_TRAY};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Duration;

#[derive(Default)]
struct Ui {
    window: HWND,
    messages: VecDeque<(u32, usize, isize)>,
    chosen: VecDeque<i32>,
    icons: Vec<(u32, u32, String, String, HANDLE)>,
    loaded_icons: Vec<(usize, i32, i32)>,
    failed_icons: Vec<usize>,
    released_icons: Vec<HANDLE>,
    shell_present: bool,
    icon_size: i32,
    menu: Vec<(u32, String)>,
    tracked: bool,
    posted: Vec<u32>,
    quits: usize,
    destroyed: usize,
    defaults: usize,
    mutex: HANDLE,
    error: u32,
    random: i32,
    closed: usize,
    /// Configuration app fakes: events (name, manual reset), signalled handles, jobs, launches and waits.
    events: Vec<(String, i32)>,
    events_fail: bool,
    set_events: Vec<HANDLE>,
    jobs: usize,
    job_limits: u32,
    assigned: Vec<(HANDLE, HANDLE)>,
    allowed: Vec<u32>,
    waits: Vec<(HANDLE, u32)>,
    app_alive: bool,
    app_closed: Vec<HANDLE>,
    spawned: Vec<String>,
    spawn_fails: bool,
    stdin: String,
    stdin_fails: bool,
}

static UI: LazyLock<Mutex<Ui>> = LazyLock::new(Default::default);
const WAIT_FOR_WORKER: u32 = 0x7fff;

fn ui() -> MutexGuard<'static, Ui> {
    UI.lock().unwrap_or_else(|e| e.into_inner())
}

fn reset() -> MutexGuard<'static, ()> {
    let g = guard();
    *ui() = Ui { window: 42, mutex: 9, icon_size: 16, ..Default::default() };
    tray::TRAY.with(|t| t.borrow_mut().take());
    g
}

unsafe extern "system" fn register_class(class: *const WNDCLASSEXW) -> u16 {
    assert_eq!(wide_string((*class).lpszClassName), "TcaNeoBridgeTray");
    1
}
unsafe extern "system" fn create_window(_: u32, _: PCWSTR, _: PCWSTR, _: u32, _: i32, _: i32, _: i32, _: i32, _: HWND, _: HANDLE, _: HANDLE, _: *const c_void) -> HWND {
    ui().window
}
unsafe extern "system" fn def_proc(_: HWND, _: u32, _: usize, _: isize) -> isize {
    ui().defaults += 1;
    7
}
unsafe extern "system" fn destroy_window(hwnd: HWND) -> ffi::BOOL {
    ui().destroyed += 1;
    tray::wndproc(hwnd, tray::WM_DESTROY, 0, 0);
    1
}
unsafe extern "system" fn get_message(msg: *mut MSG, _: HWND, _: u32, _: u32) -> ffi::BOOL {
    let next = ui().messages.pop_front();
    match next {
        Some((WAIT_FOR_WORKER, _, _)) => {
            // Test-only script step: let a pending start/stop finish, then deliver WM_STATUS.
            tray::TRAY.with(|t| t.borrow().as_ref().unwrap().controller.wait());
            (*msg).hwnd = 42;
            (*msg).message = WM_STATUS;
            1
        }
        Some((message, wparam, lparam)) => {
            (*msg).hwnd = 42;
            (*msg).message = message;
            (*msg).wParam = wparam;
            (*msg).lParam = lparam;
            1
        }
        None => 0,
    }
}
unsafe extern "system" fn translate(_: *const MSG) -> ffi::BOOL {
    1
}
unsafe extern "system" fn dispatch(msg: *const MSG) -> isize {
    tray::wndproc((*msg).hwnd, (*msg).message, (*msg).wParam, (*msg).lParam)
}
unsafe extern "system" fn post_message(_: HWND, message: u32, _: usize, _: isize) -> ffi::BOOL {
    ui().posted.push(message);
    1
}
unsafe extern "system" fn post_quit(_: i32) {
    ui().quits += 1;
}
/// A new menu after the previous one was tracked starts a fresh recording; submenus add to the current one.
unsafe extern "system" fn create_menu() -> HANDLE {
    let mut u = ui();
    if u.tracked {
        u.menu.clear();
        u.tracked = false;
    }
    5
}
unsafe extern "system" fn append_menu(_: HANDLE, flags: u32, _: usize, text: PCWSTR) -> ffi::BOOL {
    ui().menu.push((flags, wide_string(text)));
    1
}
unsafe extern "system" fn track_menu(_: HANDLE, _: u32, x: i32, y: i32, _: i32, _: HWND, _: *const c_void) -> ffi::BOOL {
    assert_eq!((x, y), (10, 20));
    // A status message arriving while the modal menu runs must not hit the borrowed tray.
    tray::wndproc(42, WM_STATUS, 0, 0);
    let mut u = ui();
    u.tracked = true;
    u.chosen.pop_front().unwrap_or(0)
}
unsafe extern "system" fn destroy_menu(_: HANDLE) -> ffi::BOOL {
    1
}
unsafe extern "system" fn set_foreground(_: HWND) -> ffi::BOOL {
    1
}
unsafe extern "system" fn cursor_pos(point: *mut POINT) -> ffi::BOOL {
    *point = POINT { x: 10, y: 20 };
    1
}
unsafe extern "system" fn load_icon(instance: HANDLE, name: PCWSTR) -> HANDLE {
    assert_eq!((instance, name as usize), (0, 32512));
    3
}
unsafe extern "system" fn load_image(instance: HANDLE, name: PCWSTR, kind: u32, width: i32, height: i32, flags: u32) -> HANDLE {
    assert_eq!((instance, kind, flags), (1, 1, 0));
    let id = name as usize;
    let mut u = ui();
    u.loaded_icons.push((id, width, height));
    if u.failed_icons.contains(&id) { 0 } else { 100 + id as HANDLE }
}
unsafe extern "system" fn system_metrics(index: i32) -> i32 {
    assert!([49, 50].contains(&index));
    ui().icon_size
}
unsafe extern "system" fn destroy_icon(icon: HANDLE) -> ffi::BOOL {
    let mut u = ui();
    assert!(!u.shell_present, "remove the notification icon before releasing its handles");
    assert!(!u.released_icons.contains(&icon), "icon released twice");
    u.released_icons.push(icon);
    1
}
unsafe extern "system" fn register_message(name: PCWSTR) -> u32 {
    assert_eq!(wide_string(name), "TaskbarCreated");
    0xc123
}
unsafe extern "system" fn notify_icon(message: u32, data: *const NOTIFYICONDATAW) -> ffi::BOOL {
    let d = &*data;
    let mut u = ui();
    if message == 0 { u.shell_present = true; }
    if message == 2 { u.shell_present = false; }
    u.icons.push((message, d.uFlags, wide_string(d.szTip.as_ptr()), wide_string(d.szInfo.as_ptr()), d.hIcon));
    1
}
unsafe extern "system" fn module_handle(_: PCWSTR) -> HANDLE {
    1
}
unsafe extern "system" fn create_mutex(_: *const c_void, _: ffi::BOOL, name: PCWSTR) -> HANDLE {
    assert_eq!(wide_string(name), app::MUTEX);
    ui().mutex
}
unsafe extern "system" fn system_error() -> u32 {
    ui().error
}
unsafe extern "system" fn system_close(_: HANDLE) -> ffi::BOOL {
    ui().closed += 1;
    1
}
unsafe extern "system" fn random(_: HANDLE, buffer: *mut u8, length: u32, _: u32) -> i32 {
    std::ptr::write_bytes(buffer, 0xab, length as usize);
    ui().random
}

fn ui_api() -> UiApi {
    UiApi {
        register_class,
        create_window,
        def_proc,
        destroy_window,
        get_message,
        translate,
        dispatch,
        post_message,
        post_quit,
        create_menu,
        append_menu,
        track_menu,
        destroy_menu,
        set_foreground,
        cursor_pos,
        load_icon,
        load_image,
        system_metrics,
        destroy_icon,
        register_message,
        notify_icon,
        module_handle,
    }
}

unsafe extern "system" fn create_event(_: *const c_void, manual: ffi::BOOL, initial: ffi::BOOL, name: PCWSTR) -> HANDLE {
    assert_eq!(initial, 0);
    let mut u = ui();
    u.events.push((wide_string(name), manual));
    if u.events_fail { 0 } else { 60 + u.events.len() as HANDLE }
}
unsafe extern "system" fn set_event(event: HANDLE) -> ffi::BOOL {
    ui().set_events.push(event);
    1
}
unsafe extern "system" fn create_job(_: *const c_void, name: PCWSTR) -> HANDLE {
    assert!(name.is_null());
    ui().jobs += 1;
    71
}
unsafe extern "system" fn set_job(job: HANDLE, class: u32, info: *const c_void, length: u32) -> ffi::BOOL {
    assert_eq!((job, class, length as usize), (71, 9, std::mem::size_of::<ffi::JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()));
    ui().job_limits = (*info.cast::<ffi::JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).BasicLimitInformation.LimitFlags;
    1
}
unsafe extern "system" fn assign(job: HANDLE, process: HANDLE) -> ffi::BOOL {
    ui().assigned.push((job, process));
    1
}
unsafe extern "system" fn allow_foreground(pid: u32) -> ffi::BOOL {
    ui().allowed.push(pid);
    1
}
unsafe extern "system" fn wait_app(handle: HANDLE, milliseconds: u32) -> u32 {
    let mut u = ui();
    u.waits.push((handle, milliseconds));
    if u.app_alive { ffi::WAIT_TIMEOUT } else { ffi::WAIT_OBJECT_0 }
}
unsafe extern "system" fn close_app_handle(handle: HANDLE) -> ffi::BOOL {
    ui().app_closed.push(handle);
    1
}

fn config_app_api() -> ConfigAppApi {
    ConfigAppApi { create_event, set_event, create_job, set_job, assign, allow_foreground, wait: wait_app, close: close_app_handle }
}

struct AppInput;

impl Write for AppInput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut u = ui();
        if u.stdin_fails {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        u.stdin.push_str(std::str::from_utf8(bytes).unwrap());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fake_spawn(exe: &Path) -> io::Result<(Launched, Box<dyn Write + Send>)> {
    let mut u = ui();
    u.spawned.push(exe.file_name().unwrap().to_string_lossy().into_owned());
    if u.spawn_fails {
        return Err(io::ErrorKind::NotFound.into());
    }
    u.app_alive = true;
    Ok((Launched { process: 77, pid: 4242, child: None }, Box::new(AppInput)))
}

fn system() -> SystemApi {
    SystemApi { create_mutex, last_error: system_error, close: system_close, random }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-tray-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn apis() -> Apis {
    Apis {
        system: system(),
        hid: hid_api(),
        device: device_api(),
        ui: ui_api(),
        driver: driver_api(),
        config_app: config_app_api(),
        spawn: fake_spawn,
        runner: neo_daemon::hidhide::real_runner,
        pnputil: fake_pnputil,
        launch: fake_launch,
        system32: temp("system32"),
        program_files: temp("no-hidhide"),
        local_app_data: Some(temp("localappdata")),
        pipe_name: "p".into(),
        bind: "127.0.0.1:0".into(),
        period: Duration::from_millis(1),
    }
}

/// tasklist reports Steam running; reg has no Steam folder.
fn fake_launch() -> neo_daemon::process::Runner {
    Box::new(|tool: &std::path::Path, _: &[String]| match tool.file_name().unwrap().to_str().unwrap() {
        "tasklist.exe" => Ok((Some(0), "\"steam.exe\",\"1234\"".into())),
        _ => Ok((Some(1), String::new())),
    })
}

fn fake_pnputil() -> neo_daemon::process::Runner {
    pnputil(std::sync::Arc::new(Mutex::new(Some("neohid.inf".into()))))
}

fn args(extra: &[&str]) -> Vec<String> {
    ["neo-daemon"].iter().chain(extra).map(|s| s.to_string()).collect()
}

fn lparam(event: u32) -> isize {
    event as isize
}

#[test]
fn options_parse_and_reject() {
    assert_eq!(app::options(&args(&[]), "dflt".into()).unwrap(), Options { data: "dflt".into(), source: Source::Tca, autostart: false, self_test: false });
    let parsed = app::options(&args(&["--self-test", "--data", "d", "--source", "validation", "--autostart", "--source", "tca"]), "dflt".into()).unwrap();
    assert_eq!(parsed, Options { data: "d".into(), source: Source::Tca, autostart: true, self_test: true });
    for bad in [&["--source"][..], &["--source", "x"], &["--data"], &["--what"]] {
        assert_eq!(app::options(&args(bad), "dflt".into()).unwrap_err(), app::USAGE);
    }
    assert!(app::self_test());

    let _ = (SystemApi::real(), UiApi::real(), Apis::real(), ConfigAppApi::real());
}

#[test]
fn default_data_lives_in_local_app_data_whatever_the_launch_folder() {
    let exe = PathBuf::from(r"C:\Tools\tca-neo-bridge\neo-daemon.exe");
    let base = PathBuf::from(r"C:\Users\pilot\AppData\Local");
    assert_eq!(app::default_data(Some(&base), &exe), base.join("tca-neo-bridge"));
    assert_eq!(app::default_data(None, &exe), PathBuf::from(r"C:\Tools\tca-neo-bridge\data"));
    let _ = app::pnputil_runner();
    let _ = app::launch_runner();
}
#[test]
fn token_requires_system_randomness() {
    let _g = reset();
    assert_eq!(app::token(&system()).unwrap(), "ab".repeat(24));
    ui().random = -1;
    assert_eq!(app::token(&system()), None);
}

#[test]
fn menu_entries_follow_status() {
    let enabled = |status: Status, driver| tray::menu_items(&status, driver, false).iter().filter(|i| i.0.is_some()).map(|i| i.2).collect::<Vec<_>>();
    assert_eq!(enabled(Status::Idle, (None, false)), [true, false, false, true, true, true]);
    assert_eq!(enabled(Status::Emulating, (Some(true), false)), [false, true, true, true, false, true]);
    assert_eq!(enabled(Status::Starting, (None, false)), [false, false, false, true, false, false]);
    assert_eq!(enabled(Status::Error("x".into()), (Some(false), false)), [true, false, false, true, true, true]);
    assert_eq!(enabled(Status::Idle, (Some(true), true)), [true, false, false, true, false, true]);
    let pause = |paused| tray::menu_items(&Status::Emulating, (None, false), paused)[2].1;
    assert_eq!([pause(false), pause(true)], ["Pause input", "Resume input"]);
    let label = |driver| tray::menu_items(&Status::Idle, driver, false)[5].1;
    assert_eq!([label((None, false)), label((Some(false), false)), label((Some(true), false)), label((Some(true), true))],
        ["Install driver\u{2026}", "Install driver\u{2026}", "Reinstall driver\u{2026}", "Installing driver\u{2026}"]);
    assert_eq!(tray::tooltip(&Status::Idle, false), "TCA-NEO bridge \u{2014} Idle");
    assert_eq!(tray::tooltip(&Status::Idle, true), "TCA-NEO bridge \u{2014} Idle");
    assert_eq!(tray::tooltip(&Status::Emulating, true), "TCA-NEO bridge \u{2014} Emulating (input paused)");
}

#[test]
fn exit_codes_for_usage_instance_and_startup_failures() {
    let _g = reset();
    assert_eq!(app::run(&args(&["--bogus"]), apis()), 2);
    assert_eq!(app::run(&args(&["--self-test"]), apis()), 0);
    ui().error = ffi::ERROR_ALREADY_EXISTS;
    assert_eq!(app::run(&args(&[]), apis()), 3);
    ui().error = 0;
    ui().mutex = 0;
    assert_eq!(app::run(&args(&[]), apis()), 3);
    ui().mutex = 9;
    // Unwritable data directory (a file is in the way).
    let blocked = temp("blocked");
    std::fs::write(&blocked, b"x").unwrap();
    assert_eq!(app::run(&args(&["--data", blocked.to_str().unwrap()]), apis()), 4);
    // Unreadable profile.
    let corrupt = temp("corrupt");
    std::fs::create_dir_all(&corrupt).unwrap();
    std::fs::write(corrupt.join("config.json"), "{").unwrap();
    assert_eq!(app::run(&args(&["--data", corrupt.to_str().unwrap()]), apis()), 4);
    // No randomness for the page token.
    ui().random = 1;
    assert_eq!(app::run(&args(&["--data", temp("norandom").to_str().unwrap()]), apis()), 4);
    ui().random = 0;
    // Configuration server cannot bind.
    let mut unbindable = apis();
    unbindable.bind = "256.0.0.1:0".into();
    assert_eq!(app::run(&args(&["--data", temp("nobind").to_str().unwrap()]), unbindable), 4);
    // Window creation fails.
    ui().window = 0;
    assert_eq!(app::run(&args(&["--data", temp("nowindow").to_str().unwrap()]), apis()), 4);
    assert_eq!(ui().released_icons, [102, 103, 104, 105]);
    assert!(ui().icons.is_empty());
    assert!(ui().closed >= 6);
}

#[test]
fn tray_session_start_configure_menu_stop_and_quit() {
    let _g = reset();
    {
        let mut s = state();
        s.sw_result = Some(0);
        s.connect = [(1, 0)].into();
    }
    let data = temp("session");
    {
        let mut u = ui();
        u.messages = [
            (WM_TRAY, 0, lparam(0x0200)),          // mouse move: ignored
            (WM_TRAY, 0, lparam(tray::WM_LBUTTONDBLCLK)), // opens the page
            (0x0001, 0, 0),                        // unhandled → DefWindowProc
            (0xc123, 0, 0),                        // TaskbarCreated → icon re-added
            (WM_COMMAND, ID_START, 0),
        ]
        .into();
    }
    // Run the loop until the scripted messages are exhausted (GetMessage then returns 0).
    assert_eq!(app::run(&args(&["--data", data.to_str().unwrap(), "--source", "validation"]), apis()), 0);
    let u = ui();
    // Double-click starts the configuration app with the port and token on its input, never on a command line.
    assert_eq!(u.spawned, ["neo-config.exe"]);
    assert!(u.stdin.ends_with(&format!(" {}\n", "ab".repeat(24))));
    // Quit asks the app to close, waits for it, then ends its job.
    assert_eq!(u.set_events, [62]);
    assert!(u.waits.contains(&(77, config_app::QUIT_WAIT_MS)) && u.app_closed.contains(&71));
    assert_eq!(u.icons.iter().filter(|i| i.0 == 0).count(), 2);
    assert!(u.defaults >= 1);
    assert!(u.posted.contains(&WM_STATUS));
    assert_eq!(u.icons.last().unwrap().0, 2);
    assert_eq!(u.released_icons, [102, 103, 104, 105]);
}

#[test]
fn autostart_emulates_and_window_close_quits_cleanly() {
    let _g = reset();
    {
        let mut s = state();
        s.sw_result = Some(0);
        s.connect = [(1, 0)].into();
    }
    ui().messages = [(WAIT_FOR_WORKER, 0, 0), (tray::WM_CLOSE, 0, 0)].into();
    let data = temp("autostart");
    assert_eq!(app::run(&args(&["--data", data.to_str().unwrap(), "--source", "validation", "--autostart"]), apis()), 0);
    let u = ui();
    assert_eq!((u.destroyed, u.quits), (1, 1));
    assert_eq!(u.icons.last().unwrap().0, 2); // icon removed
    assert_eq!(u.icons.iter().filter(|i| i.0 == 2).count(), 1);
    assert_eq!(u.released_icons, [102, 103, 104, 105]);
    drop(u);
    assert_eq!(state().sw_closed, 1); // virtual device removed on quit
    let log = std::fs::read_to_string(data.join("neo-daemon.log")).unwrap();
    assert!(log.contains("start source=Validation") && log.contains("status Emulating") && log.contains("status Idle"));
}

#[test]
fn reentrant_messages_get_default_handling_and_status_is_requeued() {
    let _g = reset();
    let _shared = installed_tray(Source::Validation);
    let icons = ui().icons.len();
    tray::TRAY.with(|t| {
        let _held = t.borrow();
        assert_eq!(send(WM_STATUS, 0, 0), 0);
        assert_eq!(send(0x0400, 0, 0), 0);
    });
    assert_eq!(ui().icons.len(), icons); // nothing touched the borrowed tray
    tray::TRAY.with(|t| t.borrow_mut().take());
}

#[test]
fn panics_are_recorded_in_the_private_log() {
    let log = temp("panic-log").with_extension("log");
    app::log_panics(log.clone());
    assert!(std::thread::spawn(|| panic!("deliberate test panic")).join().is_err());
    let _ = std::panic::take_hook();
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.contains("panic ") && text.contains("deliberate test panic"));
}

#[test]
fn window_close_while_busy_is_ignored() {
    let _g = reset();
    let shared = installed_tray(Source::Validation);
    shared.set_status(Status::Stopping);
    assert_eq!(send(tray::WM_CLOSE, 0, 0), 0);
    assert_eq!(ui().destroyed, 0);
    tray::TRAY.with(|t| t.borrow_mut().take());
}

#[test]
fn tca_start_without_hidhide_reports_the_remedy() {
    let _g = reset();
    ui().messages = [(WM_COMMAND, ID_START, 0)].into();
    let data = temp("tca");
    assert_eq!(app::run(&args(&["--data", data.to_str().unwrap()]), apis()), 0);
    assert!(ui().posted.len() >= 2);
}

fn installed_tray(source: Source) -> std::sync::Arc<neo_daemon::shared::Shared> {
    // Build a tray through run() but stop before the loop ends: script nothing, then re-install manually.
    let shared = std::sync::Arc::new(neo_daemon::shared::Shared::new(temp("manual").join("c.json"), neo_daemon::config::defaults()));
    let deps = neo_daemon::emulation::Deps {
        hid: hid_api(),
        device: device_api(),
        pipe_name: "p".into(),
        hiding: Box::new(|| Err("Install HidHide before emulating.".into())),
        period: Duration::from_millis(1),
        device_timeout: Duration::from_millis(20),
        retry: Duration::from_millis(5),
        checks: Box::new(|| vec![neo_daemon::launch::STEAM_INPUT.to_string()]),
    };
    let notify: neo_daemon::emulation::Notify = std::sync::Arc::new(|| unsafe {
        post_message(42, WM_STATUS, 0, 0);
    });
    let controller = neo_daemon::emulation::Controller::new(shared.clone(), deps, source, notify);
    let server = neo_daemon::http::Server::start(shared.clone(), "t".into(), "127.0.0.1:0").unwrap();
    let script_dir = temp("tray-driver");
    std::fs::create_dir_all(&script_dir).unwrap();
    std::fs::write(script_dir.join("install-driver.ps1"), "# test").unwrap();
    let driver_notify: neo_daemon::emulation::Notify = std::sync::Arc::new(|| unsafe {
        post_message(42, WM_STATUS, 0, 0);
    });
    let driver = neo_daemon::driver::DriverManager::new(driver_api(), fake_pnputil(), script_dir.join("install-driver.ps1"), script_dir.join("log.txt"), driver_notify);
    let config_app = ConfigApp::new(config_app_api(), PathBuf::from(config_app::EXE), fake_spawn);
    let mut t = tray::Tray { api: ui_api(), hwnd: 42, icons: tray::IconSet::load(ui_api(), 1), taskbar_created: 0xc123, controller, server, driver, config_app, shown: None, added: false };
    t.add();
    tray::TRAY.with(|slot| *slot.borrow_mut() = Some(t));
    shared
}

fn send(message: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { tray::wndproc(42, message, wparam, lparam) }
}

fn wait_status(shared: &neo_daemon::shared::Shared, wanted: impl Fn(&Status) -> bool) {
    wait_until(|| wanted(&shared.status()));
    tray::TRAY.with(|t| t.borrow().as_ref().unwrap().controller.wait());
}

#[test]
fn menu_commands_errors_balloons_and_busy_quit() {
    let _g = reset();
    {
        let mut s = state();
        s.sw_result = Some(0);
        s.connect = [(1, 0)].into();
    }
    let shared = installed_tray(Source::Validation);
    // Right-click menu → Start.
    ui().chosen = [ID_START as i32].into();
    assert_eq!(send(WM_TRAY, 0, lparam(tray::WM_RBUTTONUP)), 0);
    wait_status(&shared, |s| *s == Status::Emulating);
    let labels: Vec<String> = ui().menu.iter().map(|m| m.1.clone()).collect();
    assert_eq!(labels, ["Start emulation", "Stop emulation", "Pause input", "", "Configure\u{2026}", "Install driver\u{2026}", "", "Quit"]);
    assert_eq!(ui().menu.iter().map(|m| m.0).collect::<Vec<_>>(), [0, 1, 1, 0x800, 0, 0, 0x800, 0]);
    send(WM_STATUS, 0, 0);
    assert!(ui().icons.last().unwrap().2.ends_with("Emulating"));
    // Pause and resume input from the menu while emulating.
    send(WM_COMMAND, tray::ID_PAUSE, 0);
    assert!(shared.paused() && ui().icons.last().unwrap().2.ends_with("Emulating (input paused)"));
    send(WM_COMMAND, tray::ID_PAUSE, 0);
    assert!(!shared.paused() && ui().icons.last().unwrap().2.ends_with("Emulating"));
    // Context-menu key → Configure (does not destroy).
    ui().chosen = [ID_CONFIGURE as i32].into();
    send(WM_TRAY, 0, lparam(tray::WM_CONTEXTMENU));
    assert_eq!(ui().spawned.len(), 1);
    // Menu dismissed.
    send(WM_TRAY, 0, lparam(tray::WM_RBUTTONUP));
    // Stop via WM_COMMAND.
    send(WM_COMMAND, ID_STOP, 0);
    wait_status(&shared, |s| *s == Status::Idle);
    // Pause is ignored unless emulating.
    send(WM_COMMAND, tray::ID_PAUSE, 0);
    assert!(!shared.paused());
    // Start failure shows one balloon per new error.
    state().sw_result = Some(-1);
    send(WM_COMMAND, ID_START, 0);
    wait_status(&shared, |s| matches!(s, Status::Error(_)));
    send(WM_STATUS, 0, 0);
    send(WM_STATUS, 0, 0);
    send(WM_STATUS, 0, 0);
    // Every start also checks the launch order: here Steam Input is reported, once per start.
    let balloons: Vec<String> = ui().icons.iter().filter(|i| i.1 & 0x10 != 0).map(|i| i.3.clone()).collect();
    assert_eq!(balloons.iter().filter(|text| text.contains("ACE COMBAT 8, so Steam remaps")).count(), 2);
    assert_eq!(balloons.len(), 3); // two launch warnings and the start error, shown once
    assert_eq!(*shared.warnings.lock().unwrap(), [neo_daemon::launch::STEAM_INPUT]);
    // Quit is refused while busy.
    shared.set_status(Status::Starting);
    send(WM_COMMAND, ID_QUIT, 0);
    assert_eq!(ui().destroyed, 0);
    shared.set_status(Status::Idle);
    send(WM_COMMAND, 99, 0);
    // Quit from the menu destroys the window (WM_DESTROY → PostQuitMessage).
    ui().chosen = [ID_QUIT as i32].into();
    send(WM_TRAY, 0, lparam(tray::WM_RBUTTONUP));
    let u = ui();
    assert_eq!((u.destroyed, u.quits), (1, 1));
    drop(u);
    assert_eq!(ui().icons.last().unwrap().0, 2);
    // Quit via WM_COMMAND too.
    send(WM_COMMAND, ID_QUIT, 0);
    assert_eq!(ui().destroyed, 2);
    tray::TRAY.with(|t| t.borrow_mut().take());
    // Without a tray, the real default procedure handles messages.
    assert_eq!(send(0x0400, 0, 0), 0);
}

#[test]
fn profile_submenu_lists_saved_profiles_and_switches_with_a_balloon() {
    let _g = reset();
    let shared = installed_tray(Source::Validation);
    // No saved profiles: no submenu.
    send(WM_TRAY, 0, lparam(tray::WM_RBUTTONUP));
    assert!(!ui().menu.iter().any(|m| m.1 == "Profile"));
    let dir = shared.profiles_dir();
    let mut attack = neo_daemon::config::defaults();
    attack.axes.roll.curve = 0.4;
    neo_daemon::profiles::save(&dir, "Attack", &attack).unwrap();
    neo_daemon::profiles::save(&dir, "Cruise", &neo_daemon::config::defaults()).unwrap();
    neo_daemon::profiles::set_active(&dir, Some("Cruise")).unwrap();
    // The submenu sits before Configure…, with the active profile checked; choosing one activates it.
    ui().chosen = [(tray::ID_PROFILE_BASE) as i32].into();
    send(WM_TRAY, 0, lparam(tray::WM_RBUTTONUP));
    let menu = ui().menu.clone();
    let at = menu.iter().position(|m| m.1 == "Profile").unwrap();
    assert_eq!((menu[at].0, &menu[at + 1].1), (0x10, &"Configure\u{2026}".to_string()));
    assert!(menu.contains(&(0, "Attack".into())) && menu.contains(&(0x8, "Cruise".into())));
    assert_eq!((shared.config().1, neo_daemon::profiles::active(&dir)), (attack, Some("Attack".into())));
    let shown = ui().icons.last().unwrap().clone();
    assert!(shown.1 & 0x10 != 0 && shown.3 == "Now flying with Attack.");
    // A profile deleted while the menu was open.
    send(WM_COMMAND, tray::ID_PROFILE_BASE + 5, 0);
    assert_eq!(ui().icons.last().unwrap().3, "That profile no longer exists.");
    tray::TRAY.with(|t| t.borrow_mut().take());
}

#[test]
fn driver_menu_installs_only_when_idle_and_shows_the_outcome() {
    let _g = reset();
    let shared = installed_tray(Source::Validation);
    shared.set_status(Status::Emulating);
    send(WM_COMMAND, tray::ID_DRIVER, 0);
    assert!(driver_fake().launched.is_empty());
    shared.set_status(Status::Idle);
    send(WM_COMMAND, tray::ID_DRIVER, 0);
    tray::TRAY.with(|t| t.borrow().as_ref().unwrap().driver.wait());
    assert_eq!(driver_fake().launched.len(), 1);
    send(WM_STATUS, 0, 0);
    let shown = ui().icons.last().unwrap().clone();
    assert!(shown.1 & 0x10 != 0 && shown.3.contains("driver is ready"));
    driver_fake().uac = Uac::Refuse;
    send(WM_COMMAND, tray::ID_DRIVER, 0);
    send(WM_STATUS, 0, 0);
    assert_eq!(ui().icons.last().unwrap().3, "The driver installation was cancelled.");
    tray::TRAY.with(|t| t.borrow_mut().take());
}

#[test]
fn status_icons_and_tooltips_change_together_and_survive_explorer_restart() {
    let _g = reset();
    let shared = installed_tray(Source::Validation);
    assert_eq!(ui().loaded_icons, [(2, 16, 16), (3, 16, 16), (4, 16, 16), (5, 16, 16)]);
    for (status, icon) in [
        (Status::Idle, 102),
        (Status::Starting, 104),
        (Status::Emulating, 103),
        (Status::Stopping, 104),
        (Status::Error("Disconnected".into()), 105),
        (Status::Idle, 102),
    ] {
        shared.set_status(status.clone());
        send(WM_STATUS, 0, 0);
        let shown = ui().icons.last().unwrap().clone();
        assert_eq!((shown.2, shown.4), (tray::tooltip(&status, false), icon));
        send(0xc123, 0, 0);
        let u = ui();
        let added = &u.icons[u.icons.len() - 2];
        assert_eq!((added.0, added.2.as_str(), added.4), (0, tray::tooltip(&status, false).as_str(), icon));
    }
    assert_eq!(ui().loaded_icons.len(), 4); // cached across refreshes and Explorer restarts
    tray::TRAY.with(|t| t.borrow_mut().take());
    assert_eq!(ui().released_icons, [102, 103, 104, 105]);
}

#[test]
fn missing_icon_resources_fall_back_without_destroying_shared_handles() {
    let _g = reset();
    ui().failed_icons = vec![2, 5];
    let shared = installed_tray(Source::Validation);
    assert_eq!(ui().icons.last().unwrap().4, 3);
    shared.set_status(Status::Error("Disconnected".into()));
    send(WM_STATUS, 0, 0);
    let shown = ui().icons.last().unwrap().clone();
    assert_eq!(shown.4, 3);
    assert!(shown.1 & 0x10 != 0);
    shared.set_status(Status::Emulating);
    send(WM_STATUS, 0, 0);
    assert_eq!(ui().icons.last().unwrap().4, 103);
    tray::TRAY.with(|t| t.borrow_mut().take());
    assert_eq!(ui().released_icons, [103, 104]);
}

#[test]
fn icon_loading_uses_windows_small_icon_dimensions_at_common_scales() {
    let _g = reset();
    for size in [16, 24, 32] { // 100%, 150%, 200%
        ui().icon_size = size;
        let icons = tray::IconSet::load(ui_api(), 1);
        assert!(ui().loaded_icons.iter().rev().take(4).all(|entry| (entry.1, entry.2) == (size, size)));
        drop(icons);
        assert_eq!(ui().released_icons, [102, 103, 104, 105]);
        ui().released_icons.clear();
    }
}

#[test]
fn configure_starts_the_app_once_then_brings_it_forward() {
    let _g = reset();
    let _shared = installed_tray(Source::Validation);
    let port = tray::TRAY.with(|t| t.borrow().as_ref().unwrap().server.port);
    assert_eq!(ui().events, [(config_app::SHOW_EVENT.to_string(), 0), (config_app::QUIT_EVENT.to_string(), 1)]);
    send(WM_TRAY, 0, lparam(tray::WM_LBUTTONDBLCLK));
    {
        let u = ui();
        assert_eq!((u.spawned.len(), u.stdin.as_str(), u.jobs, u.job_limits), (1, format!("{port} t\n").as_str(), 1, 0x2000));
        assert_eq!(u.assigned, [(71, 77)]);
    }
    // Open: the daemon passes on its foreground right, then signals the app instead of starting another.
    send(WM_COMMAND, ID_CONFIGURE, 0);
    {
        let u = ui();
        assert_eq!((u.spawned.len(), u.allowed.as_slice(), u.set_events.as_slice()), (1, &[4242][..], &[61][..]));
    }
    // Closed by the user: the next Configure… starts it again, in the same job.
    ui().app_alive = false;
    send(WM_COMMAND, ID_CONFIGURE, 0);
    {
        let u = ui();
        assert_eq!((u.spawned.len(), u.jobs, u.assigned.len()), (2, 1, 2));
    }
    // It cannot start, or cannot be handed the token: a notice says how to fix it.
    ui().app_alive = false;
    ui().spawn_fails = true;
    send(WM_COMMAND, ID_CONFIGURE, 0);
    ui().spawn_fails = false;
    ui().stdin_fails = true;
    send(WM_COMMAND, ID_CONFIGURE, 0);
    {
        let u = ui();
        let notices: Vec<&String> = u.icons.iter().map(|icon| &icon.3).filter(|info| info.contains("neo-config.exe is missing")).collect();
        assert_eq!(notices.len(), 2);
    }
    tray::TRAY.with(|t| t.borrow_mut().take());
    let u = ui();
    assert_eq!(u.set_events.last(), Some(&62));
    assert_eq!(u.app_closed, [71, 61, 62]);
}

#[test]
fn config_app_without_events_or_launches_closes_nothing_extra() {
    let _g = reset();
    ui().events_fail = true;
    let mut app = ConfigApp::new(config_app_api(), PathBuf::from(config_app::EXE), fake_spawn);
    app.close();
    drop(app);
    let u = ui();
    assert!(u.app_closed.is_empty() && u.set_events.is_empty() && u.waits.is_empty());
}

#[test]
fn real_spawn_pipes_standard_input() {
    assert_eq!(config_app::spawn(Path::new(r"C:\no\such\neo-config.exe")).err().unwrap().kind(), io::ErrorKind::NotFound);
    let cmd = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join(r"System32\cmd.exe");
    let (launched, mut input) = config_app::spawn(&cmd).unwrap();
    assert!(launched.process != 0 && launched.pid != 0);
    input.write_all(b"exit\n").unwrap();
    drop(input);
    assert!(launched.child.unwrap().wait().unwrap().success());
}