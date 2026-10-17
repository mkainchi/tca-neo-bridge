//! Tray "Install driver… / Reinstall driver…": detection through pnputil and a user-approved (UAC) run of the
//! bundled install script. Nothing is installed without that explicit menu choice.
use crate::emulation::Notify;
use crate::ffi::{self, HANDLE, HWND, SHELLEXECUTEINFOW};
use crate::process::Runner;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

const SEE_MASK_NOCLOSEPROCESS: u32 = 0x40;
const INFINITE: u32 = u32::MAX;

#[derive(Clone, Copy)]
pub struct DriverApi {
    pub shell_execute_ex: unsafe extern "system" fn(*mut SHELLEXECUTEINFOW) -> ffi::BOOL,
    pub wait: unsafe extern "system" fn(HANDLE, u32) -> u32,
    pub exit_code: unsafe extern "system" fn(HANDLE, *mut u32) -> ffi::BOOL,
    pub close: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
}

impl DriverApi {
    pub fn real() -> Self {
        Self { shell_execute_ex: ffi::ShellExecuteExW, wait: ffi::WaitForSingleObject, exit_code: ffi::GetExitCodeProcess, close: ffi::CloseHandle }
    }
}

/// Some(true/false) from the driver store; None when pnputil could not be queried.
/// Only the original INF name is matched because pnputil output is localized.
pub fn detect(runner: &Runner) -> Option<bool> {
    match runner(Path::new("pnputil.exe"), &["/enum-drivers".to_string()]) {
        Ok((Some(0), output)) => Some(output.to_ascii_lowercase().contains("neohid.inf")),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: &'static str,
    pub text: String,
    pub error: bool,
}

#[derive(Default)]
struct State {
    installed: Option<bool>,
    busy: bool,
    notice: Option<Notice>,
}

pub struct DriverManager {
    api: DriverApi,
    runner: Arc<Runner>,
    script: PathBuf,
    log: PathBuf,
    state: Arc<Mutex<State>>,
    notify: Notify,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl DriverManager {
    pub fn new(api: DriverApi, runner: Runner, script: PathBuf, log: PathBuf, notify: Notify) -> Self {
        Self { api, runner: Arc::new(runner), script, log, state: Arc::default(), notify, worker: Mutex::new(None) }
    }

    /// (installed, busy) for the menu.
    pub fn menu(&self) -> (Option<bool>, bool) {
        let state = self.state.lock().unwrap();
        (state.installed, state.busy)
    }

    pub fn take_notice(&self) -> Option<Notice> {
        self.state.lock().unwrap().notice.take()
    }

    fn spawn(&self, work: impl FnOnce() + Send + 'static) {
        self.wait();
        *self.worker.lock().unwrap() = Some(std::thread::spawn(work));
    }

    /// Wait for a pending detection or installation.
    pub fn wait(&self) {
        if let Some(worker) = self.worker.lock().unwrap().take() {
            worker.join().unwrap();
        }
    }

    pub fn detect_async(&self) {
        let (runner, state, notify) = (self.runner.clone(), self.state.clone(), self.notify.clone());
        self.spawn(move || {
            state.lock().unwrap().installed = detect(&runner);
            notify();
        });
    }

    fn finish(&self, notice: Notice) {
        self.state.lock().unwrap().notice = Some(notice);
        (self.notify)();
    }

    /// Run the bundled script elevated (Windows asks the user through UAC), then re-detect and report.
    pub fn install(&self, hwnd: HWND) {
        let reinstall = {
            let state = self.state.lock().unwrap();
            if state.busy {
                return;
            }
            state.installed == Some(true)
        };
        if !self.script.is_file() {
            return self.finish(Notice {
                title: "Driver package missing",
                text: format!("{} was not found. Use the release package, which includes the driver folder.", self.script.display()),
                error: true,
            });
        }
        let parameters = format!(
            "-NoProfile -ExecutionPolicy Bypass -File \"{}\" -Log \"{}\"{}",
            self.script.display(),
            self.log.display(),
            if reinstall { " -Reinstall" } else { "" }
        );
        let (verb, file, parameters) = (ffi::wide("runas"), ffi::wide("powershell.exe"), ffi::wide(&parameters));
        // SAFETY: SHELLEXECUTEINFOW is plain data; all-zero is valid before the fields below are set.
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.hwnd = hwnd;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.nShow = 1;
        // SAFETY: strings outlive the call; `info` is fully initialised.
        if unsafe { (self.api.shell_execute_ex)(&mut info) } == 0 || info.hProcess == 0 {
            return self.finish(Notice { title: "Driver not changed", text: "The driver installation was cancelled.".into(), error: true });
        }
        self.state.lock().unwrap().busy = true;
        (self.notify)();
        let (api, runner, state, notify, process, log) = (self.api, self.runner.clone(), self.state.clone(), self.notify.clone(), info.hProcess, self.log.clone());
        self.spawn(move || {
            let mut code = 1u32;
            // SAFETY: process handle returned by ShellExecuteExW with SEE_MASK_NOCLOSEPROCESS; closed here.
            unsafe {
                (api.wait)(process, INFINITE);
                (api.exit_code)(process, &mut code);
                (api.close)(process);
            }
            let installed = detect(&runner);
            let mut state = state.lock().unwrap();
            state.installed = installed;
            state.busy = false;
            state.notice = Some(if code == 0 {
                Notice { title: "Driver installed", text: "The virtual NEO driver is ready. Start emulation from the tray menu.".into(), error: false }
            } else {
                Notice { title: "Driver installation failed", text: format!("See {} for details.", log.display()), error: true }
            });
            drop(state);
            notify();
        });
    }
}
