//! The native configuration app (`neo-config.exe` beside the daemon): started by Configure… with the API port and
//! token on its standard input, brought to the front by a named event afterwards, and closed with the daemon. It runs
//! in a kill-on-close job, so it also ends if the daemon dies.
use crate::ffi::{self, HANDLE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, PCWSTR};
use std::ffi::c_void;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub use neo_common::config_app::{EXE, QUIT_EVENT, SHOW_EVENT};
/// How long Quit waits for the app to close itself before the job ends it.
pub const QUIT_WAIT_MS: u32 = 2000;

#[derive(Clone, Copy)]
pub struct ConfigAppApi {
    pub create_event: unsafe extern "system" fn(*const c_void, ffi::BOOL, ffi::BOOL, PCWSTR) -> HANDLE,
    pub set_event: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub create_job: unsafe extern "system" fn(*const c_void, PCWSTR) -> HANDLE,
    pub set_job: unsafe extern "system" fn(HANDLE, u32, *const c_void, u32) -> ffi::BOOL,
    pub assign: unsafe extern "system" fn(HANDLE, HANDLE) -> ffi::BOOL,
    pub allow_foreground: unsafe extern "system" fn(u32) -> ffi::BOOL,
    pub wait: unsafe extern "system" fn(HANDLE, u32) -> u32,
    pub close: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
}

impl ConfigAppApi {
    pub fn real() -> Self {
        Self {
            create_event: ffi::CreateEventW,
            set_event: ffi::SetEvent,
            create_job: ffi::CreateJobObjectW,
            set_job: ffi::SetInformationJobObject,
            assign: ffi::AssignProcessToJobObject,
            allow_foreground: ffi::AllowSetForegroundWindow,
            wait: ffi::WaitForSingleObject,
            close: ffi::CloseHandle,
        }
    }
}

/// A started app: its process handle and id, and the `Child` that owns the handle (None for test fakes).
pub struct Launched {
    pub process: HANDLE,
    pub pid: u32,
    pub child: Option<Child>,
}

/// Start the app with a piped standard input; the writer goes to that input.
pub type Spawn = fn(&Path) -> io::Result<(Launched, Box<dyn Write + Send>)>;

pub fn spawn(exe: &Path) -> io::Result<(Launched, Box<dyn Write + Send>)> {
    use std::os::windows::io::AsRawHandle;
    let mut child = Command::new(exe).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    let input = child.stdin.take().expect("stdin is piped");
    Ok((Launched { process: child.as_raw_handle() as HANDLE, pid: child.id(), child: Some(child) }, Box::new(input)))
}

pub struct ConfigApp {
    api: ConfigAppApi,
    exe: PathBuf,
    spawn: Spawn,
    show: HANDLE,
    quit: HANDLE,
    job: HANDLE,
    running: Option<Launched>,
}

impl ConfigApp {
    /// The events exist from the start, so an app that is already running can always be reached.
    pub fn new(api: ConfigAppApi, exe: PathBuf, spawn: Spawn) -> Self {
        let (show, quit) = (ffi::wide(SHOW_EVENT), ffi::wide(QUIT_EVENT));
        // SAFETY: NUL-terminated names; default security.
        let (show, quit) = unsafe { ((api.create_event)(std::ptr::null(), 0, 0, show.as_ptr()), (api.create_event)(std::ptr::null(), 1, 0, quit.as_ptr())) };
        Self { api, exe, spawn, show, quit, job: 0, running: None }
    }

    /// The running app, if it has not exited.
    fn alive(&self) -> Option<&Launched> {
        // SAFETY: the process handle stays open while `running` holds it.
        self.running.as_ref().filter(|app| unsafe { (self.api.wait)(app.process, 0) } == ffi::WAIT_TIMEOUT)
    }

    /// A job that ends its processes when its last handle closes (on Quit, or when the daemon dies).
    fn job(&mut self) -> HANDLE {
        if self.job == 0 {
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = ffi::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: anonymous job; `limits` outlives the call and its size is passed.
            unsafe {
                self.job = (self.api.create_job)(std::ptr::null(), std::ptr::null());
                let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
                (self.api.set_job)(self.job, ffi::JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS, (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(), size);
            }
        }
        self.job
    }

    /// Bring the open app to the front, or start it and hand it the port and token. An error means it could not
    /// start (the caller shows a notice).
    pub fn open(&mut self, port: u16, token: &str) -> io::Result<()> {
        if let Some(app) = self.alive() {
            // SAFETY: the daemon holds the foreground right after the tray click and passes it on before signalling.
            unsafe {
                (self.api.allow_foreground)(app.pid);
                (self.api.set_event)(self.show);
            }
            return Ok(());
        }
        self.running = None;
        let (app, mut input) = (self.spawn)(&self.exe)?;
        let job = self.job();
        // SAFETY: a fresh process handle. If the job is missing the app still exits once the API stops answering.
        unsafe { (self.api.assign)(job, app.process) };
        self.running = Some(app);
        input.write_all(format!("{port} {token}\n").as_bytes())
    }

    /// Ask the app to close, give it a moment, then end the job (which ends the app if it is still there).
    pub fn close(&mut self) {
        if let Some(app) = self.running.take() {
            // SAFETY: handles owned by this object.
            unsafe {
                (self.api.set_event)(self.quit);
                (self.api.wait)(app.process, QUIT_WAIT_MS);
            }
        }
        if self.job != 0 {
            // SAFETY: the job handle is ours; closing it ends any process left in the job.
            unsafe { (self.api.close)(self.job) };
            self.job = 0;
        }
    }
}

impl Drop for ConfigApp {
    fn drop(&mut self) {
        self.close();
        for event in [self.show, self.quit].into_iter().filter(|event| *event != 0) {
            // SAFETY: event handles created in `new`.
            unsafe { (self.api.close)(event) };
        }
    }
}
