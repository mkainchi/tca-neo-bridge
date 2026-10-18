//! Win32 fakes shared by FFI-level tests. Each test binary is its own process; tests inside one binary
//! serialise on `guard()` because the fakes use process-wide state.
#![allow(dead_code, clippy::missing_safety_doc)]
use neo_daemon::device::DeviceApi;
use neo_daemon::driver::DriverApi;
use neo_daemon::ffi::{self, GUID, HANDLE, HIDD_ATTRIBUTES, HIDP_BUTTON_CAPS, HIDP_CAPS, HIDP_VALUE_CAPS, HRESULT, PCWSTR, SECURITY_ATTRIBUTES, SP_DEVICE_INTERFACE_DATA, SW_DEVICE_CREATE_INFO};
use neo_daemon::hidhide::Runner;
use neo_daemon::hid::HidApi;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::ffi::c_void;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

extern "system" {
    fn GetThreadId(thread: HANDLE) -> u32;
    fn GetCurrentThreadId() -> u32;
}

static LOCK: Mutex<()> = Mutex::new(());

pub fn guard() -> MutexGuard<'static, ()> {
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    *state() = State::default();
    *driver_fake() = DriverFake::default();
    CANCELLED.lock().unwrap().clear();
    guard
}

#[derive(Clone)]
pub struct Interface {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub input: u16,
    pub open: bool,
    pub attributes: bool,
    pub preparsed: bool,
    pub caps: i32,
    /// Top-level collection (Generic Desktop joystick by default).
    pub usage_page: u16,
    pub usage: u16,
    pub product: Option<String>,
    pub button_caps: Vec<HIDP_BUTTON_CAPS>,
    pub value_caps: Vec<HIDP_VALUE_CAPS>,
    pub button_status: i32,
    pub value_status: i32,
    /// Fake generic reports: report[1..17] is the button bitmap, then one LE u32 per entry of `fields`.
    pub fields: Vec<(u16, u16)>,
    /// Usages HidP_GetUsages reports on top of the bitmap (out-of-range values included).
    pub extra_usages: Vec<u16>,
    pub usages_status: i32,
    /// A usage whose value read fails.
    pub failing_usage: Option<u16>,
}

impl Interface {
    pub fn new(pid: u16, input: u16) -> Self {
        Self::with_vid(0x044f, pid, input)
    }

    pub fn with_vid(vid: u16, pid: u16, input: u16) -> Self {
        Self {
            path: format!(r"\\?\hid#vid_{vid:04x}&pid_{pid:04x}#x"),
            vid,
            pid,
            input,
            open: true,
            attributes: true,
            preparsed: true,
            caps: ffi::HIDP_STATUS_SUCCESS,
            usage_page: 1,
            usage: 4,
            product: None,
            button_caps: Vec::new(),
            value_caps: Vec::new(),
            button_status: ffi::HIDP_STATUS_SUCCESS,
            value_status: ffi::HIDP_STATUS_SUCCESS,
            fields: Vec::new(),
            extra_usages: Vec::new(),
            usages_status: ffi::HIDP_STATUS_SUCCESS,
            failing_usage: None,
        }
    }

    /// A generic stick: buttons 1..=`buttons`, X/Y 0..1023, Z 0..255, Rz -128..127, an eight-way hat 0..7.
    pub fn generic(vid: u16, pid: u16, buttons: u16) -> Self {
        let mut interface = Self::with_vid(vid, pid, 64);
        interface.product = Some("Fake Stick".into());
        interface.button_caps = vec![button_range(1, buttons)];
        interface.value_caps = vec![value_range(1, 0x30, 0x31, 0, 1023, 10), value(1, 0x32, 0, 255, 8), value(1, 0x35, -128, 127, 8), value(1, 0x39, 0, 7, 4)];
        interface.fields = vec![(1, 0x30), (1, 0x31), (1, 0x32), (1, 0x35), (1, 0x39)];
        interface
    }
}

pub fn button_range(min: u16, max: u16) -> HIDP_BUTTON_CAPS {
    HIDP_BUTTON_CAPS { UsagePage: 9, IsRange: 1, UsageMin: min, UsageMax: max, ..Default::default() }
}

pub fn value(page: u16, usage: u16, min: i32, max: i32, bits: u16) -> HIDP_VALUE_CAPS {
    HIDP_VALUE_CAPS { UsagePage: page, UsageMin: usage, LogicalMin: min, LogicalMax: max, BitSize: bits, ..Default::default() }
}

pub fn value_range(page: u16, first: u16, last: u16, min: i32, max: i32, bits: u16) -> HIDP_VALUE_CAPS {
    HIDP_VALUE_CAPS { IsRange: 1, UsageMax: last, ..value(page, first, min, max, bits) }
}

/// Report for `Interface::generic`-style fakes: pressed buttons, then one raw value per field.
pub fn generic_report(length: usize, buttons: &[u16], values: &[u32]) -> Vec<u8> {
    let mut report = vec![0u8; length];
    report[0] = 1;
    for button in buttons {
        let bit = usize::from(button - 1);
        report[1 + bit / 8] |= 1 << (bit % 8);
    }
    for (index, value) in values.iter().enumerate() {
        report[17 + 4 * index..21 + 4 * index].copy_from_slice(&value.to_le_bytes());
    }
    report
}

pub enum Read {
    Data(Vec<u8>),
    Fail,
    /// Returned on every read without being consumed (a device streaming reports).
    Forever(Vec<u8>),
}

#[derive(Default)]
pub struct State {
    pub list_invalid: bool,
    pub interfaces: Vec<Interface>,
    pub detail_fail: BTreeSet<u32>,
    pub opened: HashMap<HANDLE, usize>,
    pub next_handle: HANDLE,
    pub reads: HashMap<usize, VecDeque<Read>>,
    pub closed: Vec<HANDLE>,
    pub freed_preparsed: Vec<isize>,
    pub list_destroyed: usize,
    // SwDevice
    pub sw_hr: HRESULT,
    pub sw_result: Option<HRESULT>,
    pub sw_hardware: String,
    pub sw_closed: usize,
    // Pipe
    pub sddl_fail: bool,
    pub pipe_invalid: bool,
    pub connect: VecDeque<(i32, u32)>,
    pub write_fail_after: Option<usize>,
    pub frames: Vec<Vec<u8>>,
    pub disconnects: usize,
    pub freed: usize,
    pub last_error: u32,
}

static STATE: std::sync::LazyLock<Mutex<State>> = std::sync::LazyLock::new(Default::default);

pub fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

static CANCELLED: Mutex<BTreeSet<u32>> = Mutex::new(BTreeSet::new());

fn wait_cancelled() {
    let me = unsafe { GetCurrentThreadId() };
    loop {
        if CANCELLED.lock().unwrap().remove(&me) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub fn wide_string(text: PCWSTR) -> String {
    let mut length = 0;
    unsafe {
        while *text.add(length) != 0 {
            length += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(text, length))
    }
}

pub unsafe extern "system" fn class_devs(_: *const GUID, _: PCWSTR, _: ffi::HWND, _: u32) -> HANDLE {
    if state().list_invalid { ffi::INVALID_HANDLE_VALUE } else { 77 }
}

pub unsafe extern "system" fn enum_interfaces(_: HANDLE, _: *const c_void, _: *const GUID, index: u32, data: *mut SP_DEVICE_INTERFACE_DATA) -> ffi::BOOL {
    assert_eq!((*data).cbSize as usize, std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>());
    (*data).Flags = index;
    i32::from((index as usize) < state().interfaces.len())
}

pub unsafe extern "system" fn interface_detail(_: HANDLE, data: *const SP_DEVICE_INTERFACE_DATA, detail: *mut u8, size: u32, _: *mut u32, _: *mut c_void) -> ffi::BOOL {
    let index = (*data).Flags;
    let s = state();
    if s.detail_fail.contains(&index) {
        return 0;
    }
    assert_eq!(*(detail as *const u32), 8);
    let units: Vec<u16> = s.interfaces[index as usize].path.encode_utf16().chain(Some(0)).collect();
    assert!(units.len() * 2 + 4 <= size as usize);
    std::ptr::copy_nonoverlapping(units.as_ptr() as *const u8, detail.add(4), units.len() * 2);
    1
}

pub unsafe extern "system" fn destroy_list(_: HANDLE) -> ffi::BOOL {
    state().list_destroyed += 1;
    1
}

pub unsafe extern "system" fn create_file(path: PCWSTR, _: u32, _: u32, _: *const c_void, _: u32, _: u32, _: HANDLE) -> HANDLE {
    let path = wide_string(path);
    let mut s = state();
    let Some(index) = s.interfaces.iter().position(|i| i.path == path && i.open) else { return ffi::INVALID_HANDLE_VALUE };
    s.next_handle += 1;
    let handle = 1000 + s.next_handle;
    s.opened.insert(handle, index);
    handle
}

fn interface(handle: HANDLE) -> Interface {
    let s = state();
    s.interfaces[s.opened[&handle]].clone()
}

pub unsafe extern "system" fn attributes(handle: HANDLE, out: *mut HIDD_ATTRIBUTES) -> u8 {
    let i = interface(handle);
    (*out).VendorID = i.vid;
    (*out).ProductID = i.pid;
    u8::from(i.attributes)
}

pub unsafe extern "system" fn preparsed(handle: HANDLE, out: *mut isize) -> u8 {
    *out = handle;
    u8::from(interface(handle).preparsed)
}

pub unsafe extern "system" fn free_preparsed(data: isize) -> u8 {
    state().freed_preparsed.push(data);
    1
}

pub unsafe extern "system" fn caps(data: isize, out: *mut HIDP_CAPS) -> i32 {
    let i = interface(data);
    (*out).InputReportByteLength = i.input;
    (*out).UsagePage = i.usage_page;
    (*out).Usage = i.usage;
    (*out).counts[1] = i.button_caps.len() as u16;
    (*out).counts[2] = i.value_caps.len() as u16;
    i.caps
}

pub unsafe extern "system" fn product(handle: HANDLE, buffer: *mut c_void, length: u32) -> u8 {
    let Some(text) = interface(handle).product else { return 0 };
    let units: Vec<u16> = text.encode_utf16().chain(Some(0)).take(length as usize / 2).collect();
    std::ptr::copy_nonoverlapping(units.as_ptr(), buffer as *mut u16, units.len());
    1
}

pub unsafe extern "system" fn button_caps(kind: i32, out: *mut HIDP_BUTTON_CAPS, length: *mut u16, data: isize) -> i32 {
    assert_eq!(kind, ffi::HIDP_INPUT);
    let i = interface(data);
    assert_eq!(usize::from(*length), i.button_caps.len());
    std::ptr::copy_nonoverlapping(i.button_caps.as_ptr(), out, i.button_caps.len());
    i.button_status
}

pub unsafe extern "system" fn value_caps(kind: i32, out: *mut HIDP_VALUE_CAPS, length: *mut u16, data: isize) -> i32 {
    assert_eq!(kind, ffi::HIDP_INPUT);
    let i = interface(data);
    assert_eq!(usize::from(*length), i.value_caps.len());
    std::ptr::copy_nonoverlapping(i.value_caps.as_ptr(), out, i.value_caps.len());
    i.value_status
}

pub unsafe extern "system" fn usages(_: i32, page: u16, _: u16, list: *mut u16, length: *mut u32, data: isize, report: *mut u8, report_length: u32) -> i32 {
    assert_eq!(page, 9);
    let i = interface(data);
    let report = std::slice::from_raw_parts(report, report_length as usize);
    let mut found: Vec<u16> = (0..128u16).filter(|bit| report[1 + usize::from(bit / 8)] & 1 << (bit % 8) != 0).map(|bit| bit + 1).collect();
    found.extend(&i.extra_usages);
    assert!(found.len() <= *length as usize);
    std::ptr::copy_nonoverlapping(found.as_ptr(), list, found.len());
    *length = found.len() as u32;
    i.usages_status
}

pub unsafe extern "system" fn usage_value(_: i32, page: u16, _: u16, usage: u16, value: *mut u32, data: isize, report: *mut u8, report_length: u32) -> i32 {
    let i = interface(data);
    if i.failing_usage == Some(usage) {
        return 0;
    }
    let report = std::slice::from_raw_parts(report, report_length as usize);
    let index = i.fields.iter().position(|f| *f == (page, usage)).expect("field read that the fake does not describe");
    *value = u32::from_le_bytes(report[17 + 4 * index..21 + 4 * index].try_into().unwrap());
    ffi::HIDP_STATUS_SUCCESS
}

pub unsafe extern "system" fn read_file(handle: HANDLE, buffer: *mut u8, length: u32, read: *mut u32, _: *mut c_void) -> ffi::BOOL {
    loop {
        let next = {
            let mut s = state();
            let index = s.opened[&handle];
            match s.reads.get_mut(&index) {
                Some(queue) if matches!(queue.front(), Some(Read::Forever(_))) => match queue.front() {
                    Some(Read::Forever(data)) => Some(Read::Data(data.clone())),
                    _ => unreachable!(),
                },
                Some(queue) => queue.pop_front(),
                None => None,
            }
        };
        match next {
            Some(Read::Forever(_)) => unreachable!(),
            Some(Read::Data(data)) => {
                let count = data.len().min(length as usize);
                std::ptr::copy_nonoverlapping(data.as_ptr(), buffer, count);
                *read = count as u32;
                return 1;
            }
            Some(Read::Fail) => return 0,
            None => {
                if CANCELLED.lock().unwrap().remove(&GetCurrentThreadId()) {
                    return 0;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

pub unsafe extern "system" fn cancel_io(thread: HANDLE) -> ffi::BOOL {
    CANCELLED.lock().unwrap().insert(GetThreadId(thread));
    1
}

pub unsafe extern "system" fn close(handle: HANDLE) -> ffi::BOOL {
    state().closed.push(handle);
    1
}

pub fn hid_api() -> HidApi {
    HidApi {
        get_class_devs: class_devs,
        enum_interfaces,
        interface_detail,
        destroy_list,
        create_file,
        attributes,
        product,
        preparsed,
        free_preparsed,
        caps,
        button_caps,
        value_caps,
        usages,
        usage_value,
        read_file,
        cancel_io,
        close,
    }
}

pub unsafe extern "system" fn sw_create(enumerator: PCWSTR, _: PCWSTR, info: *const SW_DEVICE_CREATE_INFO, _: u32, _: *const c_void, callback: ffi::SwCallback, context: *mut c_void, device: *mut isize) -> HRESULT {
    assert_eq!(wide_string(enumerator), neo_daemon::device::ENUMERATOR);
    let (hr, result) = {
        let mut s = state();
        s.sw_hardware = wide_string((*info).pszzHardwareIds);
        (s.sw_hr, s.sw_result)
    };
    *device = 55;
    if let (true, Some(result)) = (hr >= 0, result) {
        callback(55, result, context, std::ptr::null());
    }
    hr
}

pub unsafe extern "system" fn sw_close(_: isize) {
    state().sw_closed += 1;
}

pub unsafe extern "system" fn security(sddl: PCWSTR, _: u32, out: *mut *mut c_void, _: *mut u32) -> ffi::BOOL {
    assert!(wide_string(sddl).starts_with("D:P"));
    *out = 9 as *mut c_void;
    i32::from(!state().sddl_fail)
}

pub unsafe extern "system" fn local_free(_: *mut c_void) -> *mut c_void {
    state().freed += 1;
    std::ptr::null_mut()
}

pub unsafe extern "system" fn create_pipe(_: PCWSTR, _: u32, _: u32, _: u32, _: u32, _: u32, _: u32, _: *const SECURITY_ATTRIBUTES) -> HANDLE {
    if state().pipe_invalid { ffi::INVALID_HANDLE_VALUE } else { 66 }
}

/// Scripted (result, last error) per connect; when the script is empty, block until cancelled.
pub unsafe extern "system" fn connect(_: HANDLE, _: *mut c_void) -> ffi::BOOL {
    let next = state().connect.pop_front();
    match next {
        Some((result, error)) => {
            state().last_error = error;
            result
        }
        None => {
            wait_cancelled();
            state().last_error = 995;
            0
        }
    }
}

pub unsafe extern "system" fn disconnect(_: HANDLE) -> ffi::BOOL {
    state().disconnects += 1;
    1
}

pub unsafe extern "system" fn write(_: HANDLE, data: *const u8, length: u32, written: *mut u32, _: *mut c_void) -> ffi::BOOL {
    let mut s = state();
    if s.write_fail_after == Some(s.frames.len()) {
        s.write_fail_after = None;
        return 0;
    }
    s.frames.push(std::slice::from_raw_parts(data, length as usize).to_vec());
    *written = length;
    1
}

pub unsafe extern "system" fn last_error() -> u32 {
    state().last_error
}

pub fn device_api() -> DeviceApi {
    DeviceApi { sw_create, sw_close, security, local_free, create_pipe, connect, disconnect, write, last_error, cancel_io, close }
}

pub fn stick_report(buttons: u32, x: u16) -> Vec<u8> {
    let mut report = vec![0u8; 64];
    report[0] = 1;
    report[1..3].copy_from_slice(&x.to_le_bytes());
    report[3..5].copy_from_slice(&8192u16.to_le_bytes());
    report[5..8].copy_from_slice(&[128, 128, 8]);
    report[8..11].copy_from_slice(&buttons.to_le_bytes()[..3]);
    report
}

/// Right lever centred so tests do not trigger the split-lever High-G rule by accident.
pub fn quadrant_report(left: u16) -> Vec<u8> {
    let mut report = vec![0u8; 44];
    report[0] = 1;
    report[3..5].copy_from_slice(&left.to_le_bytes());
    report[5..7].copy_from_slice(&32768u16.to_le_bytes());
    report
}

/// HidHide CLI double that accepts a session (both TCAs present) and can fail one command.
pub fn hidhide_runner(fail: Arc<Mutex<Option<String>>>) -> Runner {
    let hidden = Arc::new(Mutex::new(BTreeSet::<String>::new()));
    let apps = Arc::new(Mutex::new(Vec::<String>::new()));
    let cloak = Arc::new(Mutex::new("--cloak-off".to_string()));
    Box::new(move |_: &Path, args: &[String]| {
        if fail.lock().unwrap().as_deref() == Some(args[0].as_str()) {
            return Ok((Some(1), String::new()));
        }
        let out = match args[0].as_str() {
            "--dev-list" => hidden.lock().unwrap().iter().map(|d| format!("--dev-hide \"{d}\"")).collect::<Vec<_>>().join("\n"),
            "--app-list" => apps.lock().unwrap().iter().map(|a| format!("--app-reg \"{a}\"")).collect::<Vec<_>>().join("\n"),
            "--cloak-state" => cloak.lock().unwrap().clone(),
            "--inv-state" => "--inv-off".into(),
            "--dev-gaming" => serde_json::json!([{"devices": [
                {"deviceInstancePath": "HID\\VID_044F&PID_0405\\A", "present": true, "gamingDevice": true},
                {"deviceInstancePath": "HID\\VID_044F&PID_0407\\B", "present": true, "gamingDevice": true},
                {"deviceInstancePath": "HID\\VID_044F&PID_B10A\\C", "present": true, "gamingDevice": true},
                {"deviceInstancePath": "HID\\VID_1234&PID_0001\\D", "present": true, "gamingDevice": true}]}])
            .to_string(),
            "--app-reg" => {
                apps.lock().unwrap().push(args[1].clone());
                String::new()
            }
            "--app-unreg" => {
                apps.lock().unwrap().retain(|a| a != &args[1]);
                String::new()
            }
            "--dev-hide" => {
                hidden.lock().unwrap().insert(args[1].clone());
                String::new()
            }
            "--dev-unhide" => {
                hidden.lock().unwrap().remove(&args[1]);
                String::new()
            }
            other => {
                *cloak.lock().unwrap() = other.to_string();
                String::new()
            }
        };
        Ok((Some(0), out))
    })
}

pub fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(std::time::Instant::now() < deadline, "condition not reached");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// UAC outcome for the fake ShellExecuteExW.
#[derive(Default, Clone, Copy, PartialEq)]
pub enum Uac {
    #[default]
    Accept,
    Refuse,
    NoProcess,
}

#[derive(Default)]
pub struct DriverFake {
    pub uac: Uac,
    pub exit: u32,
    /// While true, the fake process wait blocks (an installation in progress).
    pub hold: bool,
    pub launched: Vec<(String, String, String)>,
    pub closed: usize,
}

static DRIVER: std::sync::LazyLock<Mutex<DriverFake>> = std::sync::LazyLock::new(Default::default);

pub fn driver_fake() -> MutexGuard<'static, DriverFake> {
    DRIVER.lock().unwrap_or_else(|e| e.into_inner())
}

pub unsafe extern "system" fn shell_execute_ex(info: *mut neo_daemon::ffi::SHELLEXECUTEINFOW) -> ffi::BOOL {
    let info = &mut *info;
    let mut fake = driver_fake();
    fake.launched.push((wide_string(info.lpVerb), wide_string(info.lpFile), wide_string(info.lpParameters)));
    match fake.uac {
        Uac::Accept => {
            info.hProcess = 4242;
            1
        }
        Uac::Refuse => 0,
        Uac::NoProcess => 1,
    }
}

pub unsafe extern "system" fn wait_process(_: HANDLE, _: u32) -> u32 {
    while driver_fake().hold {
        std::thread::sleep(Duration::from_millis(1));
    }
    0
}

pub unsafe extern "system" fn exit_code(_: HANDLE, code: *mut u32) -> ffi::BOOL {
    *code = driver_fake().exit;
    1
}

pub unsafe extern "system" fn close_process(_: HANDLE) -> ffi::BOOL {
    driver_fake().closed += 1;
    1
}

pub fn driver_api() -> DriverApi {
    DriverApi { shell_execute_ex, wait: wait_process, exit_code, close: close_process }
}

/// pnputil double: the listing is shared so a test can "install" the package mid-run.
pub fn pnputil(listing: Arc<Mutex<Option<String>>>) -> Runner {
    Box::new(move |_: &Path, args: &[String]| {
        assert_eq!(args, ["/enum-drivers"]);
        match listing.lock().unwrap().clone() {
            Some(text) => Ok((Some(0), text)),
            None => Ok((Some(1), String::new())),
        }
    })
}