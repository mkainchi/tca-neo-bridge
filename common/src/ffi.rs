//! Raw Win32 declarations. Modules receive these as function-pointer tables so tests can substitute fakes
//! (the Rust equivalent of the Python app's injected `api` objects).
#![allow(non_snake_case, clippy::upper_case_acronyms)]
use std::ffi::c_void;

pub type HANDLE = isize;
pub type HWND = isize;
pub type BOOL = i32;
pub type HRESULT = i32;
pub type PCWSTR = *const u16;

pub const INVALID_HANDLE_VALUE: HANDLE = -1;
pub const GENERIC_READ: u32 = 0x8000_0000;
pub const FILE_SHARE_READ_WRITE: u32 = 3;
pub const OPEN_EXISTING: u32 = 3;
pub const ERROR_PIPE_CONNECTED: u32 = 535;
pub const ERROR_ALREADY_EXISTS: u32 = 183;
pub const HIDP_STATUS_SUCCESS: i32 = 0x0011_0000;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub const GUID_DEVINTERFACE_HID: GUID =
    GUID { data1: 0x4d1e55b2, data2: 0xf16f, data3: 0x11cf, data4: [0x88, 0xcb, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30] };

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SP_DEVICE_INTERFACE_DATA {
    pub cbSize: u32,
    pub InterfaceClassGuid: GUID,
    pub Flags: u32,
    pub Reserved: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HIDD_ATTRIBUTES {
    pub Size: u32,
    pub VendorID: u16,
    pub ProductID: u16,
    pub VersionNumber: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HIDP_CAPS {
    pub Usage: u16,
    pub UsagePage: u16,
    pub InputReportByteLength: u16,
    pub OutputReportByteLength: u16,
    pub FeatureReportByteLength: u16,
    pub Reserved: [u16; 17],
    pub counts: [u16; 10],
}

#[repr(C)]
pub struct SECURITY_ATTRIBUTES {
    pub nLength: u32,
    pub lpSecurityDescriptor: *mut c_void,
    pub bInheritHandle: BOOL,
}

#[repr(C)]
pub struct SW_DEVICE_CREATE_INFO {
    pub cbSize: u32,
    pub pszInstanceId: PCWSTR,
    pub pszzHardwareIds: PCWSTR,
    pub pszzCompatibleIds: PCWSTR,
    pub pContainerId: *const GUID,
    pub CapabilityFlags: u32,
    pub pszDeviceDescription: PCWSTR,
    pub pszDeviceLocation: PCWSTR,
    pub pSecurityDescriptor: *const c_void,
}

pub type SwCallback = unsafe extern "system" fn(isize, HRESULT, *mut c_void, PCWSTR);

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: usize,
    pub lParam: isize,
    pub time: u32,
    pub pt: POINT,
    pub private: u32,
}

pub type WNDPROC = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HANDLE,
    pub hIcon: HANDLE,
    pub hCursor: HANDLE,
    pub hbrBackground: HANDLE,
    pub lpszMenuName: PCWSTR,
    pub lpszClassName: PCWSTR,
    pub hIconSm: HANDLE,
}

#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cbSize: u32,
    pub hWnd: HWND,
    pub uID: u32,
    pub uFlags: u32,
    pub uCallbackMessage: u32,
    pub hIcon: HANDLE,
    pub szTip: [u16; 128],
    pub dwState: u32,
    pub dwStateMask: u32,
    pub szInfo: [u16; 256],
    pub uVersion: u32,
    pub szInfoTitle: [u16; 64],
    pub dwInfoFlags: u32,
    pub guidItem: GUID,
    pub hBalloonIcon: HANDLE,
}

#[repr(C)]
pub struct SHELLEXECUTEINFOW {
    pub cbSize: u32,
    pub fMask: u32,
    pub hwnd: HWND,
    pub lpVerb: PCWSTR,
    pub lpFile: PCWSTR,
    pub lpParameters: PCWSTR,
    pub lpDirectory: PCWSTR,
    pub nShow: i32,
    pub hInstApp: HANDLE,
    pub lpIDList: *mut c_void,
    pub lpClass: PCWSTR,
    pub hkeyClass: HANDLE,
    pub dwHotKey: u32,
    pub hIconOrMonitor: HANDLE,
    pub hProcess: HANDLE,
}

pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_TIMEOUT: u32 = 0x102;
pub const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;
pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
    pub PerProcessUserTimeLimit: i64,
    pub PerJobUserTimeLimit: i64,
    pub LimitFlags: u32,
    pub MinimumWorkingSetSize: usize,
    pub MaximumWorkingSetSize: usize,
    pub ActiveProcessLimit: u32,
    pub Affinity: usize,
    pub PriorityClass: u32,
    pub SchedulingClass: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    pub BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
    pub IoInfo: [u64; 6],
    pub ProcessMemoryLimit: usize,
    pub JobMemoryLimit: usize,
    pub PeakProcessMemoryUsed: usize,
    pub PeakJobMemoryUsed: usize,
}

/// The common Open and Save As dialogs (comdlg32).
#[repr(C)]
pub struct OPENFILENAMEW {
    pub lStructSize: u32,
    pub hwndOwner: HWND,
    pub hInstance: HANDLE,
    pub lpstrFilter: PCWSTR,
    pub lpstrCustomFilter: *mut u16,
    pub nMaxCustFilter: u32,
    pub nFilterIndex: u32,
    pub lpstrFile: *mut u16,
    pub nMaxFile: u32,
    pub lpstrFileTitle: *mut u16,
    pub nMaxFileTitle: u32,
    pub lpstrInitialDir: PCWSTR,
    pub lpstrTitle: PCWSTR,
    pub Flags: u32,
    pub nFileOffset: u16,
    pub nFileExtension: u16,
    pub lpstrDefExt: PCWSTR,
    pub lCustData: isize,
    pub lpfnHook: *const c_void,
    pub lpTemplateName: PCWSTR,
    pub pvReserved: *mut c_void,
    pub dwReserved: u32,
    pub FlagsEx: u32,
}

#[link(name = "comdlg32")]
extern "system" {
    pub fn GetOpenFileNameW(dialog: *mut OPENFILENAMEW) -> BOOL;
    pub fn GetSaveFileNameW(dialog: *mut OPENFILENAMEW) -> BOOL;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn WaitForSingleObject(handle: HANDLE, milliseconds: u32) -> u32;
    pub fn WaitForMultipleObjects(count: u32, handles: *const HANDLE, all: BOOL, milliseconds: u32) -> u32;
    pub fn CreateEventW(attributes: *const c_void, manual: BOOL, initial: BOOL, name: PCWSTR) -> HANDLE;
    pub fn OpenEventW(access: u32, inherit: BOOL, name: PCWSTR) -> HANDLE;
    pub fn SetEvent(event: HANDLE) -> BOOL;
    pub fn CreateJobObjectW(attributes: *const c_void, name: PCWSTR) -> HANDLE;
    pub fn SetInformationJobObject(job: HANDLE, class: u32, info: *const c_void, length: u32) -> BOOL;
    pub fn AssignProcessToJobObject(job: HANDLE, process: HANDLE) -> BOOL;
    pub fn GetExitCodeProcess(process: HANDLE, code: *mut u32) -> BOOL;
    pub fn CreateMutexW(attributes: *const c_void, owner: BOOL, name: PCWSTR) -> HANDLE;
    pub fn GetLastError() -> u32;
    pub fn CloseHandle(handle: HANDLE) -> BOOL;
    pub fn CreateFileW(name: PCWSTR, access: u32, share: u32, sa: *const c_void, disposition: u32, flags: u32, template: HANDLE) -> HANDLE;
    pub fn ReadFile(handle: HANDLE, buffer: *mut u8, length: u32, read: *mut u32, overlapped: *mut c_void) -> BOOL;
    pub fn WriteFile(handle: HANDLE, buffer: *const u8, length: u32, written: *mut u32, overlapped: *mut c_void) -> BOOL;
    pub fn CancelSynchronousIo(thread: HANDLE) -> BOOL;
    pub fn CreateNamedPipeW(name: PCWSTR, open: u32, mode: u32, instances: u32, out: u32, inp: u32, timeout: u32, sa: *const SECURITY_ATTRIBUTES) -> HANDLE;
    pub fn ConnectNamedPipe(pipe: HANDLE, overlapped: *mut c_void) -> BOOL;
    pub fn DisconnectNamedPipe(pipe: HANDLE) -> BOOL;
    pub fn LocalFree(memory: *mut c_void) -> *mut c_void;
    pub fn GetModuleHandleW(name: PCWSTR) -> HANDLE;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl: PCWSTR, revision: u32, sd: *mut *mut c_void, size: *mut u32) -> BOOL;
}

#[link(name = "setupapi")]
extern "system" {
    pub fn SetupDiGetClassDevsW(guid: *const GUID, enumerator: PCWSTR, parent: HWND, flags: u32) -> HANDLE;
    pub fn SetupDiEnumDeviceInterfaces(set: HANDLE, info: *const c_void, guid: *const GUID, index: u32, data: *mut SP_DEVICE_INTERFACE_DATA) -> BOOL;
    pub fn SetupDiGetDeviceInterfaceDetailW(set: HANDLE, data: *const SP_DEVICE_INTERFACE_DATA, detail: *mut u8, size: u32, required: *mut u32, info: *mut c_void) -> BOOL;
    pub fn SetupDiDestroyDeviceInfoList(set: HANDLE) -> BOOL;
}

#[link(name = "hid")]
extern "system" {
    pub fn HidD_GetAttributes(device: HANDLE, attributes: *mut HIDD_ATTRIBUTES) -> u8;
    pub fn HidD_GetPreparsedData(device: HANDLE, data: *mut isize) -> u8;
    pub fn HidD_FreePreparsedData(data: isize) -> u8;
    pub fn HidP_GetCaps(data: isize, caps: *mut HIDP_CAPS) -> i32;
}

#[link(name = "cfgmgr32", kind = "raw-dylib")]
extern "system" {
    pub fn SwDeviceCreate(enumerator: PCWSTR, parent: PCWSTR, info: *const SW_DEVICE_CREATE_INFO, count: u32, properties: *const c_void, callback: SwCallback, context: *mut c_void, device: *mut isize) -> HRESULT;
    pub fn SwDeviceClose(device: isize);
}

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(class: *const WNDCLASSEXW) -> u16;
    pub fn CreateWindowExW(ex: u32, class: PCWSTR, name: PCWSTR, style: u32, x: i32, y: i32, w: i32, h: i32, parent: HWND, menu: HANDLE, instance: HANDLE, param: *const c_void) -> HWND;
    pub fn DefWindowProcW(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> isize;
    pub fn DestroyWindow(hwnd: HWND) -> BOOL;
    pub fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32) -> BOOL;
    pub fn TranslateMessage(msg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(msg: *const MSG) -> isize;
    pub fn PostMessageW(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> BOOL;
    pub fn PostQuitMessage(code: i32);
    pub fn CreatePopupMenu() -> HANDLE;
    pub fn AppendMenuW(menu: HANDLE, flags: u32, id: usize, text: PCWSTR) -> BOOL;
    pub fn TrackPopupMenu(menu: HANDLE, flags: u32, x: i32, y: i32, reserved: i32, hwnd: HWND, rect: *const c_void) -> BOOL;
    pub fn DestroyMenu(menu: HANDLE) -> BOOL;
    pub fn SetForegroundWindow(hwnd: HWND) -> BOOL;
    pub fn GetCursorPos(point: *mut POINT) -> BOOL;
    pub fn LoadIconW(instance: HANDLE, name: PCWSTR) -> HANDLE;
    pub fn LoadImageW(instance: HANDLE, name: PCWSTR, kind: u32, width: i32, height: i32, flags: u32) -> HANDLE;
    pub fn GetSystemMetrics(index: i32) -> i32;
    pub fn DestroyIcon(icon: HANDLE) -> BOOL;
    pub fn RegisterWindowMessageW(name: PCWSTR) -> u32;
    pub fn AllowSetForegroundWindow(process: u32) -> BOOL;
    pub fn MessageBoxW(hwnd: HWND, text: PCWSTR, caption: PCWSTR, kind: u32) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(message: u32, data: *const NOTIFYICONDATAW) -> BOOL;
    pub fn ShellExecuteW(hwnd: HWND, op: PCWSTR, file: PCWSTR, params: PCWSTR, dir: PCWSTR, show: i32) -> HANDLE;
    pub fn ShellExecuteExW(info: *mut SHELLEXECUTEINFOW) -> BOOL;
}

#[link(name = "bcrypt")]
extern "system" {
    pub fn BCryptGenRandom(algorithm: HANDLE, buffer: *mut u8, length: u32, flags: u32) -> i32;
}

/// NUL-terminated UTF-16.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// Copy into a fixed UTF-16 field, truncating and keeping the terminator.
pub fn fill(field: &mut [u16], text: &str) {
    let units: Vec<u16> = text.encode_utf16().take(field.len() - 1).collect();
    field[..units.len()].copy_from_slice(&units);
    field[units.len()] = 0;
}
