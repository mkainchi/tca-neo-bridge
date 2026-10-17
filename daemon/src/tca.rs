//! Raw TCA HID input. Device paths are used only to open handles; they are never logged or returned.
use crate::ffi::{self, GUID, HANDLE, HIDD_ATTRIBUTES, HIDP_CAPS, PCWSTR, SP_DEVICE_INTERFACE_DATA};
use crate::neo::{QUADRANT_LEN, STICK_LEN};
use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

#[derive(Clone, Copy)]
pub struct HidApi {
    pub get_class_devs: unsafe extern "system" fn(*const GUID, PCWSTR, ffi::HWND, u32) -> HANDLE,
    pub enum_interfaces: unsafe extern "system" fn(HANDLE, *const c_void, *const GUID, u32, *mut SP_DEVICE_INTERFACE_DATA) -> ffi::BOOL,
    pub interface_detail: unsafe extern "system" fn(HANDLE, *const SP_DEVICE_INTERFACE_DATA, *mut u8, u32, *mut u32, *mut c_void) -> ffi::BOOL,
    pub destroy_list: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub create_file: unsafe extern "system" fn(PCWSTR, u32, u32, *const c_void, u32, u32, HANDLE) -> HANDLE,
    pub attributes: unsafe extern "system" fn(HANDLE, *mut HIDD_ATTRIBUTES) -> u8,
    pub preparsed: unsafe extern "system" fn(HANDLE, *mut isize) -> u8,
    pub free_preparsed: unsafe extern "system" fn(isize) -> u8,
    pub caps: unsafe extern "system" fn(isize, *mut HIDP_CAPS) -> i32,
    pub read_file: unsafe extern "system" fn(HANDLE, *mut u8, u32, *mut u32, *mut c_void) -> ffi::BOOL,
    pub cancel_io: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub close: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
}

impl HidApi {
    pub fn real() -> Self {
        Self {
            get_class_devs: ffi::SetupDiGetClassDevsW,
            enum_interfaces: ffi::SetupDiEnumDeviceInterfaces,
            interface_detail: ffi::SetupDiGetDeviceInterfaceDetailW,
            destroy_list: ffi::SetupDiDestroyDeviceInfoList,
            create_file: ffi::CreateFileW,
            attributes: ffi::HidD_GetAttributes,
            preparsed: ffi::HidD_GetPreparsedData,
            free_preparsed: ffi::HidD_FreePreparsedData,
            caps: ffi::HidP_GetCaps,
            read_file: ffi::ReadFile,
            cancel_io: ffi::CancelSynchronousIo,
            close: ffi::CloseHandle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Stick,
    Quadrant,
}

/// Classify by generic attributes and report length only (no serials, no instance paths).
pub fn role(attributes: &HIDD_ATTRIBUTES, input_length: u16) -> Option<Role> {
    match (attributes.VendorID, attributes.ProductID, usize::from(input_length)) {
        (0x044f, 0x0405 | 0x0406, STICK_LEN) => Some(Role::Stick),
        (0x044f, 0x0407, QUADRANT_LEN) => Some(Role::Quadrant),
        _ => None,
    }
}

pub struct Opened {
    pub handle: HANDLE,
    pub role: Role,
}

const DIGCF_PRESENT_INTERFACE: u32 = 0x12;

fn interface_paths(api: &HidApi) -> Result<Vec<Vec<u16>>, String> {
    // SAFETY: arguments follow the SetupAPI contracts; buffers outlive each call.
    unsafe {
        let set = (api.get_class_devs)(&ffi::GUID_DEVINTERFACE_HID, std::ptr::null(), 0, DIGCF_PRESENT_INTERFACE);
        if set == ffi::INVALID_HANDLE_VALUE {
            return Err("Windows could not list HID devices. Reconnect the controllers and retry.".into());
        }
        let mut paths = Vec::new();
        let mut index = 0;
        loop {
            let mut data = SP_DEVICE_INTERFACE_DATA { cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32, ..Default::default() };
            if (api.enum_interfaces)(set, std::ptr::null(), &ffi::GUID_DEVINTERFACE_HID, index, &mut data) == 0 {
                break;
            }
            index += 1;
            let mut detail = vec![0u8; 4096];
            detail[..4].copy_from_slice(&8u32.to_le_bytes()); // cbSize of SP_DEVICE_INTERFACE_DETAIL_DATA_W on x64
            if (api.interface_detail)(set, &data, detail.as_mut_ptr(), detail.len() as u32, std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
                continue;
            }
            let path: Vec<u16> = detail[4..].as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&u| u != 0).chain(Some(0)).collect();
            paths.push(path);
        }
        (api.destroy_list)(set);
        Ok(paths)
    }
}

fn open(api: &HidApi, path: &[u16]) -> Option<Opened> {
    // SAFETY: path is NUL-terminated; out-structures are sized as the HID API requires.
    unsafe {
        let handle = (api.create_file)(path.as_ptr(), ffi::GENERIC_READ, ffi::FILE_SHARE_READ_WRITE, std::ptr::null(), ffi::OPEN_EXISTING, 0, 0);
        if handle == ffi::INVALID_HANDLE_VALUE {
            return None;
        }
        let mut attributes = HIDD_ATTRIBUTES { Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32, ..Default::default() };
        let mut caps = HIDP_CAPS::default();
        let mut preparsed = 0isize;
        let known = (api.attributes)(handle, &mut attributes) != 0 && (api.preparsed)(handle, &mut preparsed) != 0;
        let measured = known && {
            let status = (api.caps)(preparsed, &mut caps);
            (api.free_preparsed)(preparsed);
            status == ffi::HIDP_STATUS_SUCCESS
        };
        match role(&attributes, caps.InputReportByteLength).filter(|_| measured) {
            Some(role) => Some(Opened { handle, role }),
            None => {
                (api.close)(handle);
                None
            }
        }
    }
}

/// Exactly one supported sidestick and one Quadrant Eng 1&2.
pub fn find_pair(api: &HidApi) -> Result<(Opened, Opened), String> {
    let opened: Vec<Opened> = interface_paths(api)?.iter().filter_map(|path| open(api, path)).collect();
    let (sticks, quadrants): (Vec<Opened>, Vec<Opened>) = opened.into_iter().partition(|o| o.role == Role::Stick);
    if sticks.len() == 1 && quadrants.len() == 1 {
        return Ok((sticks.into_iter().next().unwrap(), quadrants.into_iter().next().unwrap()));
    }
    for item in sticks.iter().chain(&quadrants) {
        // SAFETY: handles came from CreateFileW above.
        unsafe { (api.close)(item.handle) };
    }
    Err("Connect exactly one supported TCA sidestick and one Quadrant Eng 1&2, then start emulation again.".into())
}

/// Blocking reader thread keeping the latest valid report 01.
pub struct Reader {
    pub latest: Arc<Mutex<Option<Vec<u8>>>>,
    pub failed: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    handle: HANDLE,
    api: HidApi,
}

impl Reader {
    pub fn spawn(api: HidApi, opened: Opened) -> Self {
        let length = match opened.role {
            Role::Stick => STICK_LEN,
            Role::Quadrant => QUADRANT_LEN,
        };
        let latest = Arc::new(Mutex::new(None));
        let failed = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_latest, thread_failed, thread_stop, handle) = (latest.clone(), failed.clone(), stop.clone(), opened.handle);
        let thread = std::thread::spawn(move || {
            let mut buffer = vec![0u8; length];
            while !thread_stop.load(Ordering::SeqCst) {
                let mut read = 0u32;
                // SAFETY: buffer has `length` bytes; synchronous handle.
                let ok = unsafe { (api.read_file)(handle, buffer.as_mut_ptr(), length as u32, &mut read, std::ptr::null_mut()) } != 0;
                if !ok {
                    thread_failed.store(!thread_stop.load(Ordering::SeqCst), Ordering::SeqCst);
                    return;
                }
                if read as usize == length && buffer[0] == 1 {
                    *thread_latest.lock().unwrap() = Some(buffer.clone());
                }
            }
        });
        Self { latest, failed, stop, thread: Some(thread), handle, api }
    }

    pub fn snapshot(&self) -> Option<Vec<u8>> {
        self.latest.lock().unwrap().clone()
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let thread = self.thread.take().unwrap();
        cancel_until_finished(&thread, self.api.cancel_io);
        thread.join().unwrap();
        // SAFETY: the reader thread has exited; the handle is ours.
        unsafe { (self.api.close)(self.handle) };
    }
}

/// Repeatedly cancel blocking I/O on `thread` until it exits (a cancel can race the next ReadFile/WriteFile).
pub fn cancel_until_finished<T>(thread: &JoinHandle<T>, cancel: unsafe extern "system" fn(HANDLE) -> ffi::BOOL) {
    while !thread.is_finished() {
        // SAFETY: the join handle keeps the thread handle valid.
        unsafe { cancel(thread.as_raw_handle() as HANDLE) };
        std::thread::sleep(Duration::from_millis(2));
    }
}
