//! Start-up: the port and token from the daemon on standard input, one copy at a time, and the daemon's events
//! (show: bring the window forward; quit: close it).
use crate::ffi::{self, HANDLE, PCWSTR};
use std::ffi::c_void;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::thread::JoinHandle;

pub const MUTEX: &str = r"Local\tca-neo-bridge-config-v1";
pub use neo_common::config_app::{QUIT_EVENT, SHOW_EVENT};
/// EVENT_MODIFY_STATE | SYNCHRONIZE: wait on the event (an auto-reset event is cleared by the wait).
const EVENT_ACCESS: u32 = 0x0002 | 0x0010_0000;
const INFINITE: u32 = u32::MAX;
/// MB_OK | MB_ICONINFORMATION
const MB_INFO: u32 = 0x40;

#[derive(Clone, Copy)]
pub struct SystemApi {
    pub create_mutex: unsafe extern "system" fn(*const c_void, ffi::BOOL, PCWSTR) -> HANDLE,
    pub last_error: unsafe extern "system" fn() -> u32,
    pub open_event: unsafe extern "system" fn(u32, ffi::BOOL, PCWSTR) -> HANDLE,
    pub wait_many: unsafe extern "system" fn(u32, *const HANDLE, ffi::BOOL, u32) -> u32,
    pub close: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub message_box: unsafe extern "system" fn(ffi::HWND, PCWSTR, PCWSTR, u32) -> i32,
}

impl SystemApi {
    pub fn real() -> Self {
        Self {
            create_mutex: ffi::CreateMutexW,
            last_error: ffi::GetLastError,
            open_event: ffi::OpenEventW,
            wait_many: ffi::WaitForMultipleObjects,
            close: ffi::CloseHandle,
            message_box: ffi::MessageBoxW,
        }
    }
}

/// Where the daemon's API answers, and the token it expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub port: u16,
    pub token: String,
}

/// `<port> <token>` as the daemon writes it: a non-zero port and a 48-digit lowercase hex token.
pub fn handshake(line: &str) -> Option<Handshake> {
    let (port, token) = line.trim_end().split_once(' ')?;
    let port: u16 = port.parse().ok().filter(|port| *port != 0)?;
    (token.len() == 48 && token.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then(|| Handshake { port, token: token.into() })
}

/// Standard input's first line; empty when there is none (started from Explorer rather than the tray).
pub fn read_stdin() -> String {
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line
}

pub fn tell(api: &SystemApi, text: &str) {
    let (text, caption) = (ffi::wide(text), ffi::wide("TCA-NEO bridge"));
    // SAFETY: NUL-terminated strings outlive the modal call.
    unsafe { (api.message_box)(0, text.as_ptr(), caption.as_ptr(), MB_INFO) };
}

/// Where a crash is recorded, in the bridge's data folder beside `neo-daemon.log`.
pub const CRASH_LOG: &str = "neo-config.log";
pub const CRASHED: &str = "The configuration window closed after an internal error. The details are in neo-config.log in %LOCALAPPDATA%\\tca-neo-bridge.";

/// Record an internal error (what panicked, and where) at the end of `log` and tell the user, since the window has
/// no console and would otherwise just vanish.
pub fn crashed(api: &SystemApi, log: &Path, what: &str) {
    let _ = log.parent().map(std::fs::create_dir_all);
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(log).and_then(|mut file| writeln!(file, "{millis} {what}"));
    tell(api, CRASHED);
}

/// From the first call on, every panic is recorded and reported (after the default report on standard error).
pub fn report_panics(api: SystemApi, log: PathBuf) {
    static HOOK: Once = Once::new();
    HOOK.call_once(move || {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
            crashed(&api, &log, &info.to_string().replace('\n', " "));
        }));
    });
}

/// Set from the lifecycle thread, read by the window on its next frame.
#[derive(Debug, Default)]
pub struct Signals {
    pub show: AtomicBool,
    pub quit: AtomicBool,
}

/// Open the daemon's show and quit events; None when they are missing (no daemon started this app).
pub fn events(api: &SystemApi) -> Option<[HANDLE; 2]> {
    let open = |name: &str| {
        let name = ffi::wide(name);
        // SAFETY: NUL-terminated name.
        unsafe { (api.open_event)(EVENT_ACCESS, 0, name.as_ptr()) }
    };
    let events = [open(SHOW_EVENT), open(QUIT_EVENT)];
    if events.contains(&0) {
        for event in events.into_iter().filter(|event| *event != 0) {
            // SAFETY: a handle opened above.
            unsafe { (api.close)(event) };
        }
        return None;
    }
    Some(events)
}

/// Wait for the daemon's events until it says quit (or the wait fails), raising the matching signal each time.
/// The thread is never joined: it ends with the process.
pub fn watch(api: SystemApi, events: [HANDLE; 2], signals: Arc<Signals>, wake: Box<dyn Fn() + Send>) -> JoinHandle<()> {
    std::thread::spawn(move || loop {
        // SAFETY: two event handles that stay open for the life of the process.
        let signal = match unsafe { (api.wait_many)(2, events.as_ptr(), 0, INFINITE) } {
            ffi::WAIT_OBJECT_0 => &signals.show,
            1 => &signals.quit,
            _ => break,
        };
        signal.store(true, Ordering::SeqCst);
        wake();
        if signals.quit.load(Ordering::SeqCst) {
            break;
        }
    })
}
