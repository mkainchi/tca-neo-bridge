//! Entry point: single instance, profile, configuration server, emulation controller and tray loop.
use crate::{config, hid, labels};
use crate::device::DeviceApi;
use crate::driver::{DriverApi, DriverManager};
use crate::emulation::{Controller, Deps, Notify, Source};
use crate::ffi::{self, HANDLE, MSG, WNDCLASSEXW};
use crate::hidhide::{self, Hiding, Runner};
use crate::http::{self, Server};
use crate::protocol;
use crate::shared::Shared;
use crate::hid::HidApi;
use crate::tray::{self, Tray, UiApi, WM_STATUS};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Experimental release line: its own name, mutex and data folder, so it never touches a stable install.
pub const PRODUCT: &str = "Any-TM-HOTAS bridge (experimental)";
pub const MUTEX: &str = r"Local\any-tm-hotas-bridge-owner-v1";

#[derive(Clone, Copy)]
pub struct SystemApi {
    pub create_mutex: unsafe extern "system" fn(*const c_void, ffi::BOOL, ffi::PCWSTR) -> HANDLE,
    pub last_error: unsafe extern "system" fn() -> u32,
    pub close: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub random: unsafe extern "system" fn(HANDLE, *mut u8, u32, u32) -> i32,
}

impl SystemApi {
    pub fn real() -> Self {
        Self { create_mutex: ffi::CreateMutexW, last_error: ffi::GetLastError, close: ffi::CloseHandle, random: ffi::BCryptGenRandom }
    }
}

pub struct Apis {
    pub system: SystemApi,
    pub hid: HidApi,
    pub device: DeviceApi,
    pub ui: UiApi,
    pub driver: DriverApi,
    pub runner: fn() -> Runner,
    pub pnputil: fn() -> Runner,
    pub program_files: PathBuf,
    pub local_app_data: Option<PathBuf>,
    pub pipe_name: String,
    pub bind: String,
    pub period: Duration,
    /// How long --probe listens for live reports.
    pub probe_wait: Duration,
}

impl Apis {
    pub fn real() -> Self {
        Self {
            system: SystemApi::real(),
            hid: HidApi::real(),
            device: DeviceApi::real(),
            ui: UiApi::real(),
            driver: DriverApi::real(),
            runner: hidhide::real_runner,
            pnputil: pnputil_runner,
            program_files: PathBuf::from(std::env::var_os("ProgramFiles").unwrap_or(r"C:\Program Files".into())),
            local_app_data: std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            pipe_name: protocol::PIPE_NAME.into(),
            bind: "127.0.0.1:0".into(),
            period: Duration::from_millis(10),
            probe_wait: Duration::from_millis(1500),
        }
    }
}

pub const USAGE: &str = "usage: neo-daemon [--data <dir>] [--source hotas|validation] [--autostart] [--self-test] [--probe]";

#[derive(Debug, PartialEq)]
pub struct Options {
    pub data: PathBuf,
    pub source: Source,
    pub autostart: bool,
    pub self_test: bool,
    pub probe: bool,
}

pub const APP_FOLDER: &str = "any-tm-hotas-bridge";
/// Data folder of the stable TCA-NEO bridge: read once to import its profile, never written.
pub const STABLE_FOLDER: &str = "tca-neo-bridge";

/// Profile, correction log and logs: `%LOCALAPPDATA%\any-tm-hotas-bridge`, whatever folder the daemon is started from;
/// `data` beside the executable only if LOCALAPPDATA is unavailable.
pub fn default_data(local_app_data: Option<&Path>, exe: &Path) -> PathBuf {
    local_app_data.map_or_else(|| exe.with_file_name("data"), |base| base.join(APP_FOLDER))
}

/// pnputil can be slow on systems with many staged drivers.
pub fn pnputil_runner() -> Runner {
    crate::process::runner(Duration::from_secs(60))
}

pub fn options(args: &[String], default_data: PathBuf) -> Result<Options, String> {
    let mut result = Options { data: default_data, source: Source::Hotas, autostart: false, self_test: false, probe: false };
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--self-test" => result.self_test = true,
            "--autostart" => result.autostart = true,
            "--probe" => result.probe = true,
            _ => match (arg.as_str(), args.next().map(String::as_str)) {
                ("--data", Some(dir)) => result.data = PathBuf::from(dir),
                ("--source", Some("hotas" | "tca")) => result.source = Source::Hotas,
                ("--source", Some("validation")) => result.source = Source::Validation,
                _ => return Err(USAGE.into()),
            },
        }
    }
    Ok(result)
}

/// Page assets reference each other and the profile validates.
pub fn assets_ok(profile: &config::Config, index: &str, script: &str, style: &str) -> bool {
    config::validate(profile).is_ok() && index.contains("app.js") && index.contains("style.css") && !script.is_empty() && !style.is_empty()
}

/// Embedded page and defaults are consistent (used by the coverage self-test of the real binary).
pub fn self_test() -> bool {
    assets_ok(&config::defaults(), http::INDEX, http::SCRIPT, http::STYLE)
}

/// First run: copy the stable bridge's profile and corrections, migrated, when this data folder has no profile.
/// Returns what happened, for the log.
pub fn import_stable(stable: &Path, data: &Path) -> &'static str {
    if data.join("config.json").exists() {
        return "kept";
    }
    let found = std::fs::read_to_string(stable.join("config.json")).ok().and_then(|text| config::parse(&text).ok());
    let Some(profile) = found else { return "none" };
    let recordings = labels::load(&stable.join("label-map.json")).unwrap_or_default();
    match config::save(&data.join("config.json"), &profile).and_then(|()| labels::save(&data.join("label-map.json"), &recordings)) {
        Ok(()) => "imported",
        Err(_) => "failed",
    }
}

/// --probe: write the controller report to `probe.txt` in the data folder and open it. Exit 0, or 4 on failure.
pub fn probe(apis: &Apis, data: &Path) -> i32 {
    let path = data.join("probe.txt");
    let written = std::fs::create_dir_all(data).map_err(|e| e.to_string()).and_then(|()| hid::probe(apis.hid, apis.probe_wait)).and_then(|text| std::fs::write(&path, text).map_err(|e| e.to_string()));
    if written.is_err() {
        return 4;
    }
    let (verb, file) = (ffi::wide("open"), ffi::wide(&path.to_string_lossy()));
    // SAFETY: NUL-terminated strings that outlive the call.
    unsafe { (apis.ui.shell_execute)(0, verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
    0
}

pub fn token(api: &SystemApi) -> Option<String> {
    let mut bytes = [0u8; 24];
    // SAFETY: BCRYPT_USE_SYSTEM_PREFERRED_RNG with a caller buffer.
    (unsafe { (api.random)(0, bytes.as_mut_ptr(), bytes.len() as u32, 2) } == 0).then(|| bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Record panics (message and source location only) in the private log before the default handler runs.
pub fn log_panics(path: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crate::shared::append(&path, &format!("panic {info}"));
        previous(info);
    }));
}

/// Exit codes: 0 normal, 2 usage, 3 already running, 4 profile/startup failure.
pub fn run(args: &[String], apis: Apis) -> i32 {
    let exe = std::env::current_exe().unwrap_or_default();
    let options = match options(args, default_data(apis.local_app_data.as_deref(), &exe)) {
        Ok(options) => options,
        Err(_) => return 2,
    };
    if options.self_test {
        return 4 * i32::from(!self_test());
    }
    if options.probe {
        return probe(&apis, &options.data);
    }
    let name = ffi::wide(MUTEX);
    // SAFETY: NUL-terminated name.
    let mutex = unsafe { (apis.system.create_mutex)(std::ptr::null(), 1, name.as_ptr()) };
    // SAFETY: reads this thread's last error immediately after CreateMutexW.
    if mutex == 0 || unsafe { (apis.system.last_error)() } == ffi::ERROR_ALREADY_EXISTS {
        // SAFETY: closing a null handle is harmless; otherwise it is ours.
        unsafe { (apis.system.close)(mutex) };
        return 3;
    }
    let code = owned(options, &apis, &exe);
    // SAFETY: mutex handle from CreateMutexW.
    unsafe { (apis.system.close)(mutex) };
    code
}

fn owned(options: Options, apis: &Apis, exe_path: &Path) -> i32 {
    let created = std::fs::create_dir_all(&options.data).is_ok();
    let imported = apis.local_app_data.as_ref().map(|base| base.join(STABLE_FOLDER)).filter(|stable| *stable != options.data).map(|stable| import_stable(&stable, &options.data));
    let started = Some(()).filter(|()| created).and_then(|()| config::load(&options.data.join("config.json")).ok()).zip(token(&apis.system));
    let Some((profile, token)) = started else { return 4 };
    let log = options.data.join("neo-daemon.log");
    log_panics(log.clone());
    crate::shared::append(&log, &format!("start source={:?} import={}", options.source, imported.unwrap_or("skipped")));
    let shared = Arc::new(Shared::new(options.data.join("config.json"), profile).with_log(log).with_hid(apis.hid));
    let Ok(server) = Server::start(shared.clone(), token, &apis.bind) else { return 4 };
    let ui = apis.ui;
    let hwnd = Arc::new(AtomicIsize::new(0));
    let notify_hwnd = hwnd.clone();
    // SAFETY: PostMessageW to our own window (or to nothing before it exists).
    let notify: Notify = Arc::new(move || unsafe {
        (ui.post_message)(notify_hwnd.load(Ordering::SeqCst), WM_STATUS, 0, 0);
    });
    let (runner, program_files, data) = (apis.runner, apis.program_files.clone(), options.data.clone());
    let exe = exe_path.to_string_lossy().into_owned();
    let driver_notify = notify.clone();
    let deps = Deps {
        hid: apis.hid,
        device: apis.device,
        pipe_name: apis.pipe_name.clone(),
        hiding: Box::new(move || Hiding::new(hidhide::installation(&program_files), runner(), &data, std::slice::from_ref(&exe))),
        period: apis.period,
        device_timeout: Duration::from_secs(10),
    };
    let controller = Controller::new(shared, deps, options.source, notify);
    let (class, title) = (ffi::wide("AnyTmHotasBridgeTray"), ffi::wide(PRODUCT));
    // SAFETY: class/title buffers outlive window creation; wndproc is a valid WNDPROC.
    unsafe {
        let instance = (ui.module_handle)(std::ptr::null());
        let icons = tray::IconSet::load(ui, instance);
        let window_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: tray::wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: 0,
        };
        (ui.register_class)(&window_class);
        let window = (ui.create_window)(0, class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, 0, 0, instance, std::ptr::null());
        if window == 0 {
            controller.shutdown();
            return 4;
        }
        hwnd.store(window, Ordering::SeqCst);
        let taskbar = ffi::wide("TaskbarCreated");
        let mut tray = Tray {
            api: ui,
            hwnd: window,
            icons,
            taskbar_created: (ui.register_message)(taskbar.as_ptr()),
            controller,
            server,
            driver: DriverManager::new(
                apis.driver,
                (apis.pnputil)(),
                exe_path.with_file_name("driver").join("install-driver.ps1"),
                options.data.join("driver-install.log"),
                driver_notify,
            ),
            shown: None,
            added: false,
        };
        tray.add();
        tray.driver.detect_async();
        if options.autostart {
            tray.controller.start();
        }
        tray::TRAY.with(|slot| *slot.borrow_mut() = Some(tray));
        let mut message = MSG::default();
        while (ui.get_message)(&mut message, 0, 0, 0) > 0 {
            (ui.translate)(&message);
            (ui.dispatch)(&message);
        }
    }
    // Drop outside the RefCell borrow: shell calls during cleanup can be re-entrant.
    let tray = tray::TRAY.with(|slot| slot.borrow_mut().take());
    drop(tray);
    0
}
