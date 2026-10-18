//! Runs the real executable so `main` is exercised (coverage collects child-process profiles).
use neo_daemon::ffi::{self, HANDLE, PCWSTR};
use std::process::Command;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(name: PCWSTR, file: HANDLE, flags: u32) -> HANDLE;
    fn FreeLibrary(module: HANDLE) -> i32;
    fn EnumResourceNamesW(module: HANDLE, kind: PCWSTR, callback: unsafe extern "system" fn(HANDLE, PCWSTR, PCWSTR, isize) -> i32, context: isize) -> i32;
    fn FindResourceW(module: HANDLE, name: PCWSTR, kind: PCWSTR) -> HANDLE;
    fn LoadResource(module: HANDLE, resource: HANDLE) -> HANDLE;
    fn LockResource(resource: HANDLE) -> *const u8;
    fn SizeofResource(module: HANDLE, resource: HANDLE) -> u32;
}

struct Resources(HANDLE);

impl Drop for Resources {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0) };
    }
}

unsafe extern "system" fn collect_icon_ids(_: HANDLE, _: PCWSTR, name: PCWSTR, context: isize) -> i32 {
    (*(context as *mut Vec<usize>)).push(name as usize);
    1
}

unsafe fn resource(module: HANDLE, id: usize, kind: usize) -> Vec<u8> {
    let info = FindResourceW(module, id as PCWSTR, kind as PCWSTR);
    assert_ne!(info, 0);
    let size = SizeofResource(module, info);
    assert_ne!(size, 0);
    let handle = LoadResource(module, info);
    assert_ne!(handle, 0);
    let bytes = LockResource(handle);
    assert!(!bytes.is_null());
    std::slice::from_raw_parts(bytes, size as usize).to_vec()
}

#[test]
fn executable_embeds_all_icon_frames_with_the_unbadged_bridge_first() {
    let name = ffi::wide(env!("CARGO_BIN_EXE_neo-daemon"));
    // Load as data only: this must not start the daemon or acquire its singleton mutex.
    let module = Resources(unsafe { LoadLibraryExW(name.as_ptr(), 0, 2) });
    assert_ne!(module.0, 0);
    let mut ids = Vec::<usize>::new();
    unsafe { assert_ne!(EnumResourceNamesW(module.0, 14 as PCWSTR, collect_icon_ids, &mut ids as *mut _ as isize), 0) };
    assert_eq!(ids, [1, 2, 3, 4, 5]);
    let files: [&[u8]; 5] = [
        include_bytes!("../assets/bridge.ico"),
        include_bytes!("../assets/idle.ico"),
        include_bytes!("../assets/emulating.ico"),
        include_bytes!("../assets/busy.ico"),
        include_bytes!("../assets/error.ico"),
    ];
    let word = |bytes: &[u8], offset| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    let dword = |bytes: &[u8], offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    for (id, file) in ids.into_iter().zip(files) {
        let group = unsafe { resource(module.0, id, 14) }; // RT_GROUP_ICON
        assert_eq!((word(&group, 0), word(&group, 2), word(&group, 4)), (0, 1, 7));
        for (index, size) in [16, 20, 24, 32, 48, 64, 0].into_iter().enumerate() {
            let entry = 6 + 14 * index;
            assert_eq!((group[entry], group[entry + 1], word(&group, entry + 6)), (size, size, 32));
            let image = unsafe { resource(module.0, word(&group, entry + 12) as usize, 3) }; // RT_ICON
            let ico_entry = 6 + 16 * index;
            let offset = dword(file, ico_entry + 12);
            let length = dword(file, ico_entry + 8);
            assert_eq!(image, file[offset..offset + length]);
        }
        for size in [16, 24, 32] {
            let icon = unsafe { ffi::LoadImageW(module.0, id as PCWSTR, 1, size, size, 0) };
            assert_ne!(icon, 0, "resource {id} failed to load at {size}px");
            unsafe { assert_ne!(ffi::DestroyIcon(icon), 0) };
        }
    }
}

#[test]
fn binary_self_test_and_usage_exit_codes() {
    let exe = env!("CARGO_BIN_EXE_neo-daemon");
    assert_eq!(Command::new(exe).arg("--self-test").status().unwrap().code(), Some(0));
    assert_eq!(Command::new(exe).arg("--bogus").status().unwrap().code(), Some(2));
}
