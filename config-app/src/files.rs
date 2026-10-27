//! Files the profile menu reads and writes: Windows' own Open and Save As dialogs for import and export, and the
//! cheat sheet, written beside the other temporary files and opened in the default browser to print.
use crate::ffi::{self, OPENFILENAMEW, PCWSTR};
use std::path::{Path, PathBuf};

/// OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER
const OPEN_FLAGS: u32 = 0x1000 | 0x0800 | 0x0008 | 0x0008_0000;
const SAVE_FLAGS: u32 = 0x0002 | 0x0800 | 0x0008 | 0x0008_0000;
/// Longest path the dialogs return.
const MAX_PATH: usize = 1024;

#[derive(Clone, Copy)]
pub struct FileApi {
    pub open: unsafe extern "system" fn(*mut OPENFILENAMEW) -> ffi::BOOL,
    pub save: unsafe extern "system" fn(*mut OPENFILENAMEW) -> ffi::BOOL,
    pub shell: unsafe extern "system" fn(ffi::HWND, PCWSTR, PCWSTR, PCWSTR, PCWSTR, i32) -> ffi::HANDLE,
}

impl FileApi {
    pub fn real() -> Self {
        Self { open: ffi::GetOpenFileNameW, save: ffi::GetSaveFileNameW, shell: ffi::ShellExecuteW }
    }
}

/// A file chosen in a dialog: `save` picks the Save As dialog (suggesting `suggested`), otherwise Open. None when
/// the dialog was cancelled.
fn choose(api: &FileApi, save: bool, suggested: &str) -> Option<PathBuf> {
    let filter: Vec<u16> = "Profiles (*.json)\0*.json\0All files\0*.*\0\0".encode_utf16().collect();
    let extension = ffi::wide("json");
    let mut file = vec![0u16; MAX_PATH];
    let start: Vec<u16> = suggested.encode_utf16().take(MAX_PATH - 1).collect();
    file[..start.len()].copy_from_slice(&start);
    // SAFETY: OPENFILENAMEW is plain data; all-zero is a valid initial value.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.nFilterIndex = 1;
    dialog.lpstrFile = file.as_mut_ptr();
    dialog.nMaxFile = MAX_PATH as u32;
    dialog.lpstrDefExt = extension.as_ptr();
    dialog.Flags = if save { SAVE_FLAGS } else { OPEN_FLAGS };
    // SAFETY: every buffer the dialog reads or writes outlives this modal call.
    let chosen = unsafe { if save { (api.save)(&mut dialog) } else { (api.open)(&mut dialog) } };
    let length = file.iter().position(|unit| *unit == 0).unwrap_or(MAX_PATH);
    (chosen != 0).then(|| PathBuf::from(String::from_utf16_lossy(&file[..length])))
}

/// A profile file to import.
pub fn choose_import(api: &FileApi) -> Option<PathBuf> {
    choose(api, false, "")
}

/// Where to export the saved settings, suggesting `name`.
pub fn choose_export(api: &FileApi, name: &str) -> Option<PathBuf> {
    choose(api, true, name)
}

/// Open a file with its default program (the cheat sheet, in the browser).
pub fn open(api: &FileApi, path: &Path) {
    let (verb, file) = (ffi::wide("open"), ffi::wide(&path.to_string_lossy()));
    // SAFETY: NUL-terminated strings outlive the call.
    unsafe { (api.shell)(0, verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
}
