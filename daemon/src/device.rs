//! Virtual NEO lifetime (SwDeviceCreate, bound to driver/neohid.inf) and the report pipe feeding the driver.
use crate::ffi::{self, HANDLE, HRESULT, PCWSTR, SECURITY_ATTRIBUTES, SW_DEVICE_CREATE_INFO};
use crate::neo::{neutral, Report};
use crate::protocol;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// The SDK parses VID_/PID_ from the HID interface path; hidclass derives the child device ID from the
/// parent's, so the software-device enumerator carries the NEO identity (like USB\VID_044F&PID_2800).
pub const ENUMERATOR: &str = "VID_044F&PID_2800";
pub const HARDWARE_ID: &str = r"neohid\NeoVirtualHid";
pub const INSTANCE_ID: &str = "neo0";
/// Owner full access; LocalService and SYSTEM (UMDF host accounts) read-only. Local clients only.
pub const PIPE_SDDL: &str = "D:P(A;;GA;;;OW)(A;;GR;;;LS)(A;;GR;;;SY)";
const E_ACCESSDENIED: HRESULT = 0x8007_0005_u32 as i32;

#[derive(Clone, Copy)]
pub struct DeviceApi {
    pub sw_create: unsafe extern "system" fn(PCWSTR, PCWSTR, *const SW_DEVICE_CREATE_INFO, u32, *const c_void, ffi::SwCallback, *mut c_void, *mut isize) -> HRESULT,
    pub sw_close: unsafe extern "system" fn(isize),
    pub security: unsafe extern "system" fn(PCWSTR, u32, *mut *mut c_void, *mut u32) -> ffi::BOOL,
    pub local_free: unsafe extern "system" fn(*mut c_void) -> *mut c_void,
    pub create_pipe: unsafe extern "system" fn(PCWSTR, u32, u32, u32, u32, u32, u32, *const SECURITY_ATTRIBUTES) -> HANDLE,
    pub connect: unsafe extern "system" fn(HANDLE, *mut c_void) -> ffi::BOOL,
    pub disconnect: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub write: unsafe extern "system" fn(HANDLE, *const u8, u32, *mut u32, *mut c_void) -> ffi::BOOL,
    pub last_error: unsafe extern "system" fn() -> u32,
    pub cancel_io: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub close: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
}

impl DeviceApi {
    pub fn real() -> Self {
        Self {
            sw_create: ffi::SwDeviceCreate,
            sw_close: ffi::SwDeviceClose,
            security: ffi::ConvertStringSecurityDescriptorToSecurityDescriptorW,
            local_free: ffi::LocalFree,
            create_pipe: ffi::CreateNamedPipeW,
            connect: ffi::ConnectNamedPipe,
            disconnect: ffi::DisconnectNamedPipe,
            write: ffi::WriteFile,
            last_error: ffi::GetLastError,
            cancel_io: ffi::CancelSynchronousIo,
            close: ffi::CloseHandle,
        }
    }
}

type Completion = (Mutex<Option<HRESULT>>, Condvar);

/// SwDeviceCreate completion callback.
///
/// # Safety
/// `context` must point to the live `Completion` owned by `SoftwareDevice::create`.
pub unsafe extern "system" fn created(_device: isize, result: HRESULT, context: *mut c_void, _instance: PCWSTR) {
    let completion = &*(context as *const Completion);
    *completion.0.lock().unwrap() = Some(result);
    completion.1.notify_all();
}

pub fn multi_sz(items: &[&str]) -> Vec<u16> {
    items.iter().flat_map(|item| ffi::wide(item)).chain(Some(0)).collect()
}

/// Exists while this value lives; Windows also removes it if the daemon process exits.
pub struct SoftwareDevice {
    handle: isize,
    api: DeviceApi,
}

impl SoftwareDevice {
    pub fn create(api: DeviceApi, timeout: Duration) -> Result<Self, String> {
        let (enumerator, parent, instance, description) =
            (ffi::wide(ENUMERATOR), ffi::wide(r"HTREE\ROOT\0"), ffi::wide(INSTANCE_ID), ffi::wide("Virtual NEO HID (TCA-NEO bridge)"));
        let hardware = multi_sz(&[HARDWARE_ID]);
        let info = SW_DEVICE_CREATE_INFO {
            cbSize: std::mem::size_of::<SW_DEVICE_CREATE_INFO>() as u32,
            pszInstanceId: instance.as_ptr(),
            pszzHardwareIds: hardware.as_ptr(),
            pszzCompatibleIds: std::ptr::null(),
            pContainerId: std::ptr::null(),
            CapabilityFlags: 0x1 | 0x2 | 0x8, // removable, silent install, driver required
            pszDeviceDescription: description.as_ptr(),
            pszDeviceLocation: std::ptr::null(),
            pSecurityDescriptor: std::ptr::null(),
        };
        let completion: Arc<Completion> = Arc::new((Mutex::new(None), Condvar::new()));
        let mut handle = 0isize;
        // SAFETY: strings and `info` outlive the call; `completion` is kept alive until the callback ran or the handle is closed.
        let hr = unsafe {
            (api.sw_create)(enumerator.as_ptr(), parent.as_ptr(), &info, 0, std::ptr::null(), created, Arc::as_ptr(&completion) as *mut c_void, &mut handle)
        };
        if hr < 0 {
            return Err(if hr == E_ACCESSDENIED {
                "Creating the virtual NEO requires administrator rights. Restart neo-daemon as administrator.".into()
            } else {
                format!("Windows refused the virtual NEO (0x{:08x}). Use Reinstall driver… in the tray menu.", hr as u32)
            });
        }
        let device = Self { handle, api };
        let (result, _) = completion.1.wait_timeout_while(completion.0.lock().unwrap(), timeout, |r| r.is_none()).unwrap();
        match *result {
            Some(code) if code >= 0 => Ok(device),
            Some(_) => Err("The virtual NEO could not start. Use Install driver… in the tray menu, then retry.".into()),
            None => Err("The virtual NEO did not appear in time. Use Install driver… (or Reinstall driver…) in the tray menu.".into()),
        }
    }
}

impl Drop for SoftwareDevice {
    fn drop(&mut self) {
        // SAFETY: handle came from SwDeviceCreate.
        unsafe { (self.api.sw_close)(self.handle) };
    }
}

/// Single-instance outbound message pipe; writes the latest report every `period`.
pub struct PipeServer {
    pub latest: Arc<Mutex<Report>>,
    pub connected: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pipe: HANDLE,
    api: DeviceApi,
}

impl PipeServer {
    pub fn start(api: DeviceApi, name: &str, period: Duration) -> Result<Self, String> {
        let sddl = ffi::wide(PIPE_SDDL);
        let mut descriptor: *mut c_void = std::ptr::null_mut();
        // SAFETY: SDDL is NUL-terminated; descriptor is freed with LocalFree below.
        if unsafe { (api.security)(sddl.as_ptr(), 1, &mut descriptor, std::ptr::null_mut()) } == 0 {
            return Err("Windows rejected the report pipe security descriptor.".into());
        }
        let attributes = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: descriptor, bInheritHandle: 0 };
        let wide_name = ffi::wide(name);
        // PIPE_ACCESS_OUTBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE; PIPE_TYPE_MESSAGE | PIPE_REJECT_REMOTE_CLIENTS.
        // SAFETY: name and attributes outlive the call.
        let pipe = unsafe { (api.create_pipe)(wide_name.as_ptr(), 0x2 | 0x8_0000, 0x4 | 0x8, 1, 4096, 0, 0, &attributes) };
        // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW.
        unsafe { (api.local_free)(descriptor) };
        if pipe == ffi::INVALID_HANDLE_VALUE {
            return Err("The report pipe is already in use. Quit other neo-daemon instances and retry.".into());
        }
        let latest = Arc::new(Mutex::new(neutral()));
        let connected = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_latest, thread_connected, thread_stop) = (latest.clone(), connected.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                // SAFETY: synchronous pipe handle owned by this server.
                let accepted = unsafe { (api.connect)(pipe, std::ptr::null_mut()) != 0 || (api.last_error)() == ffi::ERROR_PIPE_CONNECTED };
                thread_connected.store(accepted, Ordering::SeqCst);
                if accepted {
                    stream(&api, pipe, &thread_latest, &thread_stop, period);
                } else {
                    std::thread::sleep(period);
                }
                thread_connected.store(false, Ordering::SeqCst);
                // SAFETY: as above.
                unsafe { (api.disconnect)(pipe) };
            }
        });
        Ok(Self { latest, connected, stop, thread: Some(thread), pipe, api })
    }

    pub fn send(&self, report: Report) {
        *self.latest.lock().unwrap() = report;
    }
}

/// Write the latest report every `period` until the client goes away or stop is requested.
/// Drop publishes neutral before setting stop, so the last frame written is always neutral.
fn stream(api: &DeviceApi, pipe: HANDLE, latest: &Mutex<Report>, stop: &AtomicBool, period: Duration) {
    loop {
        let stopping = stop.load(Ordering::SeqCst);
        let frame = protocol::frame(&latest.lock().unwrap());
        let mut written = 0u32;
        // SAFETY: frame outlives the synchronous write.
        if unsafe { (api.write)(pipe, frame.as_ptr(), frame.len() as u32, &mut written, std::ptr::null_mut()) } == 0 || stopping {
            return;
        }
        std::thread::sleep(period);
    }
}

impl Drop for PipeServer {
    fn drop(&mut self) {
        self.send(neutral());
        self.stop.store(true, Ordering::SeqCst);
        let thread = self.thread.take().unwrap();
        crate::tca::cancel_until_finished(&thread, self.api.cancel_io);
        thread.join().unwrap();
        // SAFETY: thread exited; the pipe handle is ours.
        unsafe { (self.api.close)(self.pipe) };
    }
}
