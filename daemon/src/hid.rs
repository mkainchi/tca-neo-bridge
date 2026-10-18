//! Raw HID joystick input. Device paths are used only to open handles; they are never logged or returned.
//! The TCA Airbus pair keeps its byte-exact decoders; every other joystick is read through its HID descriptor.
use crate::ffi::{self, GUID, HANDLE, HIDD_ATTRIBUTES, HIDP_BUTTON_CAPS, HIDP_CAPS, HIDP_VALUE_CAPS, PCWSTR, SP_DEVICE_INTERFACE_DATA};
use crate::models::{self, Axis, DeviceInfo, Layout};
use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

pub const STICK_LEN: usize = 64;
pub const QUADRANT_LEN: usize = 44;
pub const NONE_FOUND: &str = "Connect a supported Thrustmaster HOTAS (or tick your controllers under Axes and settings), then start emulation again.";

#[derive(Clone, Copy)]
pub struct HidApi {
    pub get_class_devs: unsafe extern "system" fn(*const GUID, PCWSTR, ffi::HWND, u32) -> HANDLE,
    pub enum_interfaces: unsafe extern "system" fn(HANDLE, *const c_void, *const GUID, u32, *mut SP_DEVICE_INTERFACE_DATA) -> ffi::BOOL,
    pub interface_detail: unsafe extern "system" fn(HANDLE, *const SP_DEVICE_INTERFACE_DATA, *mut u8, u32, *mut u32, *mut c_void) -> ffi::BOOL,
    pub destroy_list: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub create_file: unsafe extern "system" fn(PCWSTR, u32, u32, *const c_void, u32, u32, HANDLE) -> HANDLE,
    pub attributes: unsafe extern "system" fn(HANDLE, *mut HIDD_ATTRIBUTES) -> u8,
    pub product: unsafe extern "system" fn(HANDLE, *mut c_void, u32) -> u8,
    pub preparsed: unsafe extern "system" fn(HANDLE, *mut isize) -> u8,
    pub free_preparsed: unsafe extern "system" fn(isize) -> u8,
    pub caps: unsafe extern "system" fn(isize, *mut HIDP_CAPS) -> i32,
    pub button_caps: unsafe extern "system" fn(i32, *mut HIDP_BUTTON_CAPS, *mut u16, isize) -> i32,
    pub value_caps: unsafe extern "system" fn(i32, *mut HIDP_VALUE_CAPS, *mut u16, isize) -> i32,
    pub usages: unsafe extern "system" fn(i32, u16, u16, *mut u16, *mut u32, isize, *mut u8, u32) -> i32,
    pub usage_value: unsafe extern "system" fn(i32, u16, u16, u16, *mut u32, isize, *mut u8, u32) -> i32,
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
            product: ffi::HidD_GetProductString,
            preparsed: ffi::HidD_GetPreparsedData,
            free_preparsed: ffi::HidD_FreePreparsedData,
            caps: ffi::HidP_GetCaps,
            button_caps: ffi::HidP_GetButtonCaps,
            value_caps: ffi::HidP_GetValueCaps,
            usages: ffi::HidP_GetUsages,
            usage_value: ffi::HidP_GetUsageValue,
            read_file: ffi::ReadFile,
            cancel_io: ffi::CancelSynchronousIo,
            close: ffi::CloseHandle,
        }
    }
}

/// One decoded report: button n is bit n-1, axes are raw values (0..=max of the layout axis), hat 0..7 from up clockwise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputState {
    pub buttons: u128,
    pub axes: Vec<u32>,
    pub hat: Option<u8>,
}

const DESKTOP: u16 = 0x01;
const BUTTON: u16 = 0x09;
const HAT: u16 = 0x39;
/// Axes read from generic devices, in layout order, with the names used in profiles.
pub const AXES: [(u16, u16, &str); 13] = [
    (0x01, 0x30, "x"),
    (0x01, 0x31, "y"),
    (0x01, 0x32, "z"),
    (0x01, 0x33, "rx"),
    (0x01, 0x34, "ry"),
    (0x01, 0x35, "rz"),
    (0x01, 0x36, "slider"),
    (0x01, 0x37, "dial"),
    (0x01, 0x38, "wheel"),
    (0x02, 0xba, "rudder"),
    (0x02, 0xbb, "throttle"),
    (0x02, 0xc4, "accelerator"),
    (0x02, 0xc5, "brake"),
];

/// A value field of a generic device: logical minimum, span (max - min), bit size and signedness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Value {
    pub page: u16,
    pub usage: u16,
    pub min: i64,
    pub span: u32,
    pub bits: u16,
    pub signed: bool,
}

impl Value {
    /// None for ranges HidP cannot describe usefully (empty, or a bit size outside 1..=32).
    pub fn new(cap: &HIDP_VALUE_CAPS, usage: u16) -> Option<Self> {
        let bits = cap.BitSize;
        if !(1..=32).contains(&bits) {
            return None;
        }
        let (min, max) = (i64::from(cap.LogicalMin), i64::from(cap.LogicalMax));
        let signed = min < 0;
        // Descriptors often declare an unsigned maximum that HidP returns sign-extended (0..65535 as 0..-1).
        let max = if max < min && !signed { (1i64 << bits) - 1 } else { max };
        (max > min).then(|| Self { page: cap.UsagePage, usage, min, span: (max - min) as u32, bits, signed })
    }

    /// Raw report bits as the logical value (sign-extended when the logical minimum is negative).
    pub fn logical(&self, raw: u32) -> i64 {
        let shift = 64 - u32::from(self.bits);
        if self.signed { (i64::from(raw) << shift) >> shift } else { i64::from(raw) }
    }

    /// Raw report bits → 0..=span, or None outside the logical range (a hat's null state).
    pub fn position(&self, raw: u32) -> Option<u32> {
        let offset = self.logical(raw) - self.min;
        (0..=i64::from(self.span)).contains(&offset).then_some(offset as u32)
    }

    /// Raw report bits → 0..=span, clamped.
    pub fn relative(&self, raw: u32) -> u32 {
        (self.logical(raw) - self.min).clamp(0, i64::from(self.span)) as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generic {
    pub preparsed: isize,
    pub buttons: u16,
    pub axes: Vec<Value>,
    pub hat: Option<Value>,
}

impl Generic {
    /// Read the input capabilities: Button-page usages, known axes and the first hat switch.
    pub fn build(api: &HidApi, preparsed: isize, caps: &HIDP_CAPS) -> Option<Self> {
        let mut buttons = vec![HIDP_BUTTON_CAPS::default(); usize::from(caps.counts[1])];
        let mut values = vec![HIDP_VALUE_CAPS::default(); usize::from(caps.counts[2])];
        let (mut button_count, mut value_count) = (caps.counts[1], caps.counts[2]);
        // SAFETY: buffers hold the counts HidP_GetCaps reported.
        let ok = unsafe {
            (button_count == 0 || (api.button_caps)(ffi::HIDP_INPUT, buttons.as_mut_ptr(), &mut button_count, preparsed) == ffi::HIDP_STATUS_SUCCESS)
                && (value_count == 0 || (api.value_caps)(ffi::HIDP_INPUT, values.as_mut_ptr(), &mut value_count, preparsed) == ffi::HIDP_STATUS_SUCCESS)
        };
        if !ok {
            return None;
        }
        let usages = |min: u16, max: u16, range: u8| if range != 0 { min..=max.max(min) } else { min..=min };
        let top = buttons[..usize::from(button_count)]
            .iter()
            .filter(|c| c.UsagePage == BUTTON)
            .flat_map(|c| usages(c.UsageMin, c.UsageMax, c.IsRange))
            .max()
            .unwrap_or(0)
            .min(128);
        let fields: Vec<Value> = values[..usize::from(value_count)]
            .iter()
            .flat_map(|c| usages(c.UsageMin, c.UsageMax, c.IsRange).filter_map(move |u| Value::new(c, u)))
            .collect();
        let find = |page: u16, usage: u16| fields.iter().find(|v| v.page == page && v.usage == usage).copied();
        let axes = AXES.iter().filter_map(|(page, usage, _)| find(*page, *usage)).collect();
        Some(Self { preparsed, buttons: top, axes, hat: find(DESKTOP, HAT) })
    }

    pub fn layout(&self) -> Layout {
        let name = |v: &Value| AXES.iter().find(|a| a.0 == v.page && a.1 == v.usage).unwrap().2.to_string();
        Layout { buttons: self.buttons, axes: self.axes.iter().map(|v| Axis { name: name(v), max: v.span }).collect(), hat: self.hat.is_some() }
    }

    /// None when the report does not carry this collection's fields (another report id) or HidP refuses it.
    pub fn decode(&self, api: &HidApi, report: &[u8]) -> Option<InputState> {
        let mut report = report.to_vec();
        let (pointer, length) = (report.as_mut_ptr(), report.len() as u32);
        let mut state = InputState::default();
        if self.buttons > 0 {
            let mut list = [0u16; 128];
            let mut count = list.len() as u32;
            // SAFETY: list holds `count` usages; the report buffer outlives the call.
            if unsafe { (api.usages)(ffi::HIDP_INPUT, BUTTON, 0, list.as_mut_ptr(), &mut count, self.preparsed, pointer, length) } != ffi::HIDP_STATUS_SUCCESS {
                return None;
            }
            state.buttons = list[..count as usize].iter().filter(|&&u| (1..=128).contains(&u)).fold(0, |mask, &u| mask | 1u128 << (u - 1));
        }
        let read = |value: &Value| {
            let mut raw = 0u32;
            // SAFETY: as above.
            let status = unsafe { (api.usage_value)(ffi::HIDP_INPUT, value.page, 0, value.usage, &mut raw, self.preparsed, pointer, length) };
            (status == ffi::HIDP_STATUS_SUCCESS).then_some(raw)
        };
        for value in &self.axes {
            state.axes.push(value.relative(read(value)?));
        }
        if let Some(hat) = &self.hat {
            // Eight positions from up clockwise; four-position hats report the cardinals only; anything else is centred.
            state.hat = match (hat.span, hat.position(read(hat)?)) {
                (7, Some(position)) => Some(position as u8),
                (3, Some(position)) => Some(position as u8 * 2),
                _ => None,
            };
        }
        Some(state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoder {
    TcaStick,
    TcaQuadrant,
    Generic(Generic),
}

impl Decoder {
    pub fn name(&self) -> &'static str {
        match self {
            Decoder::TcaStick => "tca-stick",
            Decoder::TcaQuadrant => "tca-quadrant",
            Decoder::Generic(_) => "generic",
        }
    }

    pub fn layout(&self) -> Layout {
        match self {
            Decoder::TcaStick => models::tca_stick().layout,
            Decoder::TcaQuadrant => models::tca_quadrant().layout,
            Decoder::Generic(generic) => generic.layout(),
        }
    }

    pub fn decode(&self, api: &HidApi, report: &[u8]) -> Option<InputState> {
        match self {
            Decoder::TcaStick => tca_stick(report),
            Decoder::TcaQuadrant => tca_quadrant(report),
            Decoder::Generic(generic) => generic.decode(api, report),
        }
    }
}

fn word(report: &[u8], offset: usize) -> u32 {
    u32::from(u16::from_le_bytes([report[offset], report[offset + 1]]))
}

/// TCA sidestick report 01: roll @1, pitch @3, twist @5, slider @6, hat nibble @7, 17 inputs @8..10.
pub fn tca_stick(report: &[u8]) -> Option<InputState> {
    (report.len() == STICK_LEN && report[0] == 1).then(|| InputState {
        buttons: u128::from(u32::from_le_bytes([report[8], report[9], report[10], 0]) & 0x1ffff),
        axes: vec![word(report, 1), word(report, 3), u32::from(report[5]), u32::from(report[6])],
        hat: Some(report[7] & 15).filter(|h| *h < 8),
    })
}

/// TCA quadrant report 01: buttons @1..2, detent states @9 (low nibble), add-ons @12..13; levers @3 and @5.
pub fn tca_quadrant(report: &[u8]) -> Option<InputState> {
    (report.len() == QUADRANT_LEN && report[0] == 1).then(|| InputState {
        buttons: u128::from(word(report, 1) | u32::from(report[9] & 15) << 16 | (word(report, 12) & 0x7ff) << 20),
        axes: vec![word(report, 3), word(report, 5)],
        hat: None,
    })
}

pub struct Opened {
    pub handle: HANDLE,
    pub preparsed: isize,
    pub info: DeviceInfo,
    pub decoder: Decoder,
    pub length: usize,
}

impl Opened {
    pub fn close(self, api: &HidApi) {
        // SAFETY: both came from HidD_GetPreparsedData / CreateFileW and are released once.
        unsafe {
            (api.free_preparsed)(self.preparsed);
            (api.close)(self.handle);
        }
    }
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

/// Product string as the device reports it (never a serial number), limited to 64 printable characters.
fn product(api: &HidApi, handle: HANDLE) -> Option<String> {
    let mut buffer = [0u16; 127];
    // SAFETY: the buffer length is passed in bytes.
    if unsafe { (api.product)(handle, buffer.as_mut_ptr() as *mut c_void, 254) } == 0 {
        return None;
    }
    let text: String = String::from_utf16_lossy(&buffer[..buffer.iter().position(|&u| u == 0).unwrap_or(buffer.len())]).chars().filter(|c| !c.is_control()).collect();
    Some(text.trim().chars().take(64).collect::<String>()).filter(|t| !t.is_empty())
}

/// Joystick or game pad top-level collection (Generic Desktop 0x04 / 0x05).
pub fn joystick(caps: &HIDP_CAPS) -> bool {
    caps.UsagePage == DESKTOP && matches!(caps.Usage, 0x04 | 0x05)
}

fn decoder(api: &HidApi, id: &str, preparsed: isize, caps: &HIDP_CAPS) -> Option<Decoder> {
    let length = usize::from(caps.InputReportByteLength);
    match id {
        models::TCA_STICK => (length == STICK_LEN).then_some(Decoder::TcaStick),
        models::TCA_QUADRANT => (length == QUADRANT_LEN).then_some(Decoder::TcaQuadrant),
        _ => Generic::build(api, preparsed, caps).map(Decoder::Generic),
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
        let measured = known && (api.caps)(preparsed, &mut caps) == ffi::HIDP_STATUS_SUCCESS;
        let (vid, pid) = (attributes.VendorID, attributes.ProductID);
        let id = models::base_id(vid, pid);
        let input = (measured && joystick(&caps) && !(vid == models::THRUSTMASTER && pid == models::NEO_PID)).then_some(()).and_then(|()| decoder(api, &id, preparsed, &caps));
        let Some(decoder) = input else {
            if known {
                (api.free_preparsed)(preparsed);
            }
            (api.close)(handle);
            return None;
        };
        let name = models::model(vid, pid).map(|(_, name)| name.to_string()).or_else(|| product(api, handle)).unwrap_or_else(|| format!("Joystick {vid:04X}:{pid:04X}"));
        let info = DeviceInfo { id, name, vid, pid, layout: decoder.layout() };
        Some(Opened { handle, preparsed, info, decoder, length: usize::from(caps.InputReportByteLength) })
    }
}

/// Every present joystick, numbered so identical devices get distinct ids.
pub fn scan(api: &HidApi) -> Result<Vec<Opened>, String> {
    let mut opened: Vec<Opened> = interface_paths(api)?.iter().filter_map(|path| open(api, path)).collect();
    let mut infos: Vec<DeviceInfo> = opened.iter().map(|o| o.info.clone()).collect();
    models::number(&mut infos);
    for (item, info) in opened.iter_mut().zip(infos) {
        item.info = info;
    }
    Ok(opened)
}

/// Present joysticks, for the configuration page (handles are closed again).
pub fn list(api: &HidApi) -> Result<Vec<DeviceInfo>, String> {
    Ok(scan(api)?.into_iter().map(|opened| {
        let info = opened.info.clone();
        opened.close(api);
        info
    }).collect())
}

/// The session's devices: those `wanted` selects (every Thrustmaster joystick when it is empty); at least one.
pub fn find(api: &HidApi, wanted: &[String]) -> Result<Vec<Opened>, String> {
    let (keep, other): (Vec<Opened>, Vec<Opened>) = scan(api)?.into_iter().partition(|o| selected(wanted, &o.info));
    for item in other {
        item.close(api);
    }
    if keep.is_empty() {
        return Err(NONE_FOUND.into());
    }
    Ok(keep)
}

/// Whether a present device belongs to the session (an explicit list names exact ids, "#2" included).
pub fn selected(wanted: &[String], info: &DeviceInfo) -> bool {
    if wanted.is_empty() {
        models::auto(info.vid, info.pid)
    } else {
        wanted.contains(&info.id)
    }
}

/// Blocking reader thread keeping the latest report that decodes, and its raw bytes for diagnostics.
pub struct Reader {
    pub info: DeviceInfo,
    pub latest: Arc<Mutex<Option<InputState>>>,
    pub raw: Arc<Mutex<Option<Vec<u8>>>>,
    pub failed: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    handle: HANDLE,
    preparsed: isize,
    api: HidApi,
}

impl Reader {
    pub fn spawn(api: HidApi, opened: Opened) -> Self {
        let Opened { handle, preparsed, info, decoder, length } = opened;
        let latest = Arc::new(Mutex::new(None));
        let raw = Arc::new(Mutex::new(None));
        let failed = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_latest, thread_raw, thread_failed, thread_stop) = (latest.clone(), raw.clone(), failed.clone(), stop.clone());
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
                if read as usize == length {
                    *thread_raw.lock().unwrap() = Some(buffer.clone());
                    if let Some(state) = decoder.decode(&api, &buffer) {
                        *thread_latest.lock().unwrap() = Some(state);
                    }
                }
            }
        });
        Self { info, latest, raw, failed, stop, thread: Some(thread), handle, preparsed, api }
    }

    pub fn snapshot(&self) -> Option<InputState> {
        self.latest.lock().unwrap().clone()
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let thread = self.thread.take().unwrap();
        cancel_until_finished(&thread, self.api.cancel_io);
        thread.join().unwrap();
        // SAFETY: the reader thread has exited; the handle and preparsed data are ours.
        unsafe {
            (self.api.free_preparsed)(self.preparsed);
            (self.api.close)(self.handle);
        }
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

/// Text report for testers: identities, layouts and one live sample per joystick. No paths or serial numbers.
pub fn probe(api: HidApi, wait: Duration) -> Result<String, String> {
    let found = scan(&api)?;
    let mut text = String::from("Any-TM-HOTAS bridge probe. Device paths and serial numbers are never included.\n");
    if found.is_empty() {
        text.push_str("No joysticks found. Connect the controllers (and close HidHide's configuration client), then run --probe again.\n");
        return Ok(text);
    }
    let decoders: Vec<&'static str> = found.iter().map(|o| o.decoder.name()).collect();
    let readers: Vec<Reader> = found.into_iter().map(|o| Reader::spawn(api, o)).collect();
    std::thread::sleep(wait);
    for (reader, decoder) in readers.iter().zip(decoders) {
        let info = &reader.info;
        let layout = &info.layout;
        let axes: Vec<String> = layout.axes.iter().map(|a| format!("{} 0..{}", a.name, a.max)).collect();
        text.push_str(&format!(
            "\ndevice {} \"{}\" {:04X}:{:04X} decoder={decoder} default={}\n  buttons {}, hat {}, axes: {}\n",
            info.id,
            info.name,
            info.vid,
            info.pid,
            if models::auto(info.vid, info.pid) { "yes" } else { "no (tick it under Axes and settings)" },
            layout.buttons,
            if layout.hat { "yes" } else { "no" },
            if axes.is_empty() { "none".into() } else { axes.join(", ") },
        ));
        match reader.snapshot() {
            Some(state) => {
                let pressed: Vec<String> = (0..128).filter(|i| state.buttons & 1 << i != 0).map(|i| (i + 1).to_string()).collect();
                let values: Vec<String> = layout.axes.iter().zip(&state.axes).map(|(a, v)| format!("{}={v}", a.name)).collect();
                let hat = state.hat.map_or("centred", |h| crate::neo::HAT_NAMES[usize::from(h)]);
                text.push_str(&format!("  live: buttons [{}], hat {hat}, {}\n", pressed.join(" "), values.join(" ")));
            }
            None => text.push_str("  live: no decodable report yet (move a control while probing)\n"),
        }
        if let Some(raw) = reader.raw.lock().unwrap().as_ref() {
            text.push_str(&format!("  raw: {}\n", raw.iter().map(|b| format!("{b:02x}")).collect::<String>()));
        }
    }
    Ok(text)
}
