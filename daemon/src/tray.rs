//! Notification-area icon and menu: Start/Stop emulation, Pause/Resume input, Configure…, Install/Reinstall driver…, Quit.
use crate::driver::{DriverManager, Notice};
use crate::emulation::Controller;
use crate::ffi::{self, HANDLE, HWND, MSG, NOTIFYICONDATAW, PCWSTR, POINT, WNDCLASSEXW};
use crate::http::Server;
use crate::shared::Status;
use std::cell::RefCell;
use std::ffi::c_void;

pub const WM_TRAY: u32 = 0x8001;
pub const WM_STATUS: u32 = 0x8002;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_LBUTTONDBLCLK: u32 = 0x0203;
pub const WM_RBUTTONUP: u32 = 0x0205;
pub const WM_CONTEXTMENU: u32 = 0x007b;
pub const ID_START: usize = 1;
pub const ID_STOP: usize = 2;
pub const ID_CONFIGURE: usize = 3;
pub const ID_QUIT: usize = 4;
pub const ID_DRIVER: usize = 5;
pub const ID_PAUSE: usize = 6;
/// Saved profiles in the Profile submenu: this plus their index in name order.
pub const ID_PROFILE_BASE: usize = 100;
const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIF_MESSAGE_ICON_TIP: u32 = 0x1 | 0x2 | 0x4;
const NIF_INFO: u32 = 0x10;
const MF_STRING: u32 = 0;
const MF_GRAYED: u32 = 1;
const MF_SEPARATOR: u32 = 0x800;
const MF_CHECKED: u32 = 0x8;
const MF_POPUP: u32 = 0x10;
const TPM_RETURNCMD_RIGHT: u32 = 0x100 | 0x8;

#[derive(Clone, Copy)]
pub struct UiApi {
    pub register_class: unsafe extern "system" fn(*const WNDCLASSEXW) -> u16,
    pub create_window: unsafe extern "system" fn(u32, PCWSTR, PCWSTR, u32, i32, i32, i32, i32, HWND, HANDLE, HANDLE, *const c_void) -> HWND,
    pub def_proc: unsafe extern "system" fn(HWND, u32, usize, isize) -> isize,
    pub destroy_window: unsafe extern "system" fn(HWND) -> ffi::BOOL,
    pub get_message: unsafe extern "system" fn(*mut MSG, HWND, u32, u32) -> ffi::BOOL,
    pub translate: unsafe extern "system" fn(*const MSG) -> ffi::BOOL,
    pub dispatch: unsafe extern "system" fn(*const MSG) -> isize,
    pub post_message: unsafe extern "system" fn(HWND, u32, usize, isize) -> ffi::BOOL,
    pub post_quit: unsafe extern "system" fn(i32),
    pub create_menu: unsafe extern "system" fn() -> HANDLE,
    pub append_menu: unsafe extern "system" fn(HANDLE, u32, usize, PCWSTR) -> ffi::BOOL,
    pub track_menu: unsafe extern "system" fn(HANDLE, u32, i32, i32, i32, HWND, *const c_void) -> ffi::BOOL,
    pub destroy_menu: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub set_foreground: unsafe extern "system" fn(HWND) -> ffi::BOOL,
    pub cursor_pos: unsafe extern "system" fn(*mut POINT) -> ffi::BOOL,
    pub load_icon: unsafe extern "system" fn(HANDLE, PCWSTR) -> HANDLE,
    pub load_image: unsafe extern "system" fn(HANDLE, PCWSTR, u32, i32, i32, u32) -> HANDLE,
    pub system_metrics: unsafe extern "system" fn(i32) -> i32,
    pub destroy_icon: unsafe extern "system" fn(HANDLE) -> ffi::BOOL,
    pub register_message: unsafe extern "system" fn(PCWSTR) -> u32,
    pub notify_icon: unsafe extern "system" fn(u32, *const NOTIFYICONDATAW) -> ffi::BOOL,
    pub shell_execute: unsafe extern "system" fn(HWND, PCWSTR, PCWSTR, PCWSTR, PCWSTR, i32) -> HANDLE,
    pub module_handle: unsafe extern "system" fn(PCWSTR) -> HANDLE,
}

impl UiApi {
    pub fn real() -> Self {
        Self {
            register_class: ffi::RegisterClassExW,
            create_window: ffi::CreateWindowExW,
            def_proc: ffi::DefWindowProcW,
            destroy_window: ffi::DestroyWindow,
            get_message: ffi::GetMessageW,
            translate: ffi::TranslateMessage,
            dispatch: ffi::DispatchMessageW,
            post_message: ffi::PostMessageW,
            post_quit: ffi::PostQuitMessage,
            create_menu: ffi::CreatePopupMenu,
            append_menu: ffi::AppendMenuW,
            track_menu: ffi::TrackPopupMenu,
            destroy_menu: ffi::DestroyMenu,
            set_foreground: ffi::SetForegroundWindow,
            cursor_pos: ffi::GetCursorPos,
            load_icon: ffi::LoadIconW,
            load_image: ffi::LoadImageW,
            system_metrics: ffi::GetSystemMetrics,
            destroy_icon: ffi::DestroyIcon,
            register_message: ffi::RegisterWindowMessageW,
            notify_icon: ffi::Shell_NotifyIconW,
            shell_execute: ffi::ShellExecuteW,
            module_handle: ffi::GetModuleHandleW,
        }
    }
}

/// Private LoadImageW handles; the shared system fallback is never destroyed.
pub struct IconSet {
    api: UiApi,
    owned: [HANDLE; 4],
    fallback: HANDLE,
}

impl IconSet {
    pub fn load(api: UiApi, instance: HANDLE) -> Self {
        // SAFETY: numeric resource IDs (2..=5) in our executable; no LR_SHARED, so we own the handles.
        let owned = unsafe {
            let width = (api.system_metrics)(49); // SM_CXSMICON
            let height = (api.system_metrics)(50); // SM_CYSMICON
            [2, 3, 4, 5].map(|id| (api.load_image)(instance, id as PCWSTR, 1, width, height, 0))
        };
        // SAFETY: LoadIconW returns a shared system handle (IDI_APPLICATION).
        let fallback = unsafe { (api.load_icon)(0, 32512 as PCWSTR) };
        Self { api, owned, fallback }
    }

    pub fn for_status(&self, status: &Status) -> HANDLE {
        let index = match status {
            Status::Idle => 0,
            Status::Emulating => 1,
            Status::Starting | Status::Stopping => 2,
            Status::Error(_) => 3,
        };
        match self.owned[index] {
            0 => self.fallback,
            icon => icon,
        }
    }
}

impl Drop for IconSet {
    fn drop(&mut self) {
        for icon in self.owned.into_iter().filter(|icon| *icon != 0) {
            // SAFETY: each non-null handle was privately created by LoadImageW.
            unsafe { (self.api.destroy_icon)(icon) };
        }
    }
}

/// (command, label, enabled); `None` command is a separator. `driver` is (installed, installing).
pub fn menu_items(status: &Status, driver: (Option<bool>, bool), paused: bool) -> Vec<(Option<usize>, &'static str, bool)> {
    let busy = matches!(status, Status::Starting | Status::Stopping);
    let driver_label = match driver {
        (_, true) => "Installing driver\u{2026}",
        (Some(true), false) => "Reinstall driver\u{2026}",
        _ => "Install driver\u{2026}",
    };
    vec![
        (Some(ID_START), "Start emulation", !status.running()),
        (Some(ID_STOP), "Stop emulation", *status == Status::Emulating),
        (Some(ID_PAUSE), if paused { "Resume input" } else { "Pause input" }, *status == Status::Emulating),
        (None, "", false),
        (Some(ID_CONFIGURE), "Configure\u{2026}", true),
        (Some(ID_DRIVER), driver_label, !status.running() && !driver.1),
        (None, "", false),
        (Some(ID_QUIT), "Quit", !busy),
    ]
}

pub fn tooltip(status: &Status, paused: bool) -> String {
    let note = if paused && *status == Status::Emulating { " (input paused)" } else { "" };
    format!("TCA-NEO bridge \u{2014} {}{note}", status.label())
}

pub enum Action {
    Done(isize),
    Menu,
    Destroy,
    Default,
}

pub struct Tray {
    pub api: UiApi,
    pub hwnd: HWND,
    pub icons: IconSet,
    pub taskbar_created: u32,
    pub controller: Controller,
    pub server: Server,
    pub driver: DriverManager,
    pub shown: Option<Status>,
    pub added: bool,
}

impl Tray {
    fn data(&self, status: &Status) -> NOTIFYICONDATAW {
        // SAFETY: NOTIFYICONDATAW is plain data; all-zero is a valid initial value.
        let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = self.hwnd;
        data.uID = 1;
        data.uFlags = NIF_MESSAGE_ICON_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = self.icons.for_status(status);
        ffi::fill(&mut data.szTip, &tooltip(status, self.controller.shared.paused()));
        data
    }

    pub fn add(&mut self) {
        let status = self.controller.shared.status();
        // SAFETY: data is fully initialised.
        unsafe { (self.api.notify_icon)(NIM_ADD, &self.data(&status)) };
        self.added = true;
        self.shown = None;
        self.refresh();
    }

    /// Update tooltip; show a balloon once per new error.
    pub fn refresh(&mut self) {
        let status = self.controller.shared.status();
        let mut data = self.data(&status);
        if let Some(notice) = self.driver.take_notice().or_else(|| self.controller.shared.take_notice()) {
            data.uFlags |= NIF_INFO;
            data.dwInfoFlags = if notice.error { 3 } else { 1 }; // NIIF_ERROR / NIIF_INFO
            ffi::fill(&mut data.szInfoTitle, notice.title);
            ffi::fill(&mut data.szInfo, &notice.text);
        } else if let Status::Error(message) = &status {
            if self.shown.as_ref() != Some(&status) {
                data.uFlags |= NIF_INFO;
                data.dwInfoFlags = 3; // NIIF_ERROR
                ffi::fill(&mut data.szInfoTitle, "NEO emulation stopped");
                ffi::fill(&mut data.szInfo, message);
            }
        }
        // SAFETY: as above.
        unsafe { (self.api.notify_icon)(NIM_MODIFY, &data) };
        self.shown = Some(status);
    }

    pub fn configure(&self) {
        let (verb, url) = (ffi::wide("open"), ffi::wide(&self.server.url()));
        // SAFETY: NUL-terminated strings outlive the call.
        unsafe { (self.api.shell_execute)(self.hwnd, verb.as_ptr(), url.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
    }

    /// Stop emulation and remove the icon; the caller destroys the window outside the tray borrow.
    pub fn quit(&mut self) {
        self.controller.shutdown();
        self.driver.wait();
        if self.added {
            // SAFETY: as above. Remove from the shell before IconSet releases its handles.
            unsafe { (self.api.notify_icon)(NIM_DELETE, &self.data(&self.controller.shared.status())) };
            self.added = false;
        }
    }

    /// Returns true when the window must now be destroyed (Quit).
    pub fn command(&mut self, id: usize) -> bool {
        match id {
            ID_START => self.controller.start(),
            ID_STOP => self.controller.stop(),
            ID_PAUSE if self.controller.shared.status() == Status::Emulating => {
                let shared = &self.controller.shared;
                shared.set_paused(!shared.paused());
                self.refresh();
            }
            ID_CONFIGURE => self.configure(),
            id if id >= ID_PROFILE_BASE => self.switch_profile(id - ID_PROFILE_BASE),
            ID_DRIVER if !self.controller.shared.status().running() => self.driver.install(self.hwnd),
            ID_QUIT if !matches!(self.controller.shared.status(), Status::Starting | Status::Stopping) => {
                self.quit();
                return true;
            }
            _ => {}
        }
        false
    }

    /// Activate the profile chosen in the submenu (by its position in name order) and say how it went.
    fn switch_profile(&mut self, index: usize) {
        let shared = &self.controller.shared;
        let name = crate::profiles::list(&shared.profiles_dir()).get(index).cloned().ok_or_else(|| "That profile no longer exists.".to_string());
        shared.notice(match name.and_then(|name| shared.activate(&name).map(|_| name)) {
            Ok(name) => Notice { title: "Profile switched", text: format!("Now flying with {name}."), error: false },
            Err(error) => Notice { title: "Profile not switched", text: error, error: true },
        });
        self.refresh();
    }

    /// What the tray menu shows, read while the tray state is borrowed.
    pub fn menu_state(&self) -> MenuState {
        let shared = &self.controller.shared;
        let dir = shared.profiles_dir();
        MenuState { status: shared.status(), driver: self.driver.menu(), paused: shared.paused(), profiles: crate::profiles::list(&dir), active: crate::profiles::active(&dir) }
    }

    pub fn handle(&mut self, message: u32, wparam: usize, lparam: isize) -> Action {
        match message {
            WM_TRAY => match lparam as u32 & 0xffff {
                WM_RBUTTONUP | WM_CONTEXTMENU => return Action::Menu,
                WM_LBUTTONDBLCLK => self.configure(),
                _ => {}
            },
            WM_STATUS => self.refresh(),
            // `taskkill` without /F (and session end) closes the hidden window: treat it as Quit.
            WM_CLOSE if self.command(ID_QUIT) => return Action::Destroy,
            WM_CLOSE => {}
            WM_COMMAND if self.command(wparam & 0xffff) => return Action::Destroy,
            WM_COMMAND => {}
            WM_DESTROY => {
                // SAFETY: ends this thread's message loop.
                unsafe { (self.api.post_quit)(0) }
            }
            m if m == self.taskbar_created => self.add(),
            _ => return Action::Default,
        }
        Action::Done(0)
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // Also handles a failed/exhausted message loop and unwinding.
        self.quit();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuState {
    pub status: Status,
    /// Driver (installed, installing).
    pub driver: (Option<bool>, bool),
    pub paused: bool,
    pub profiles: Vec<String>,
    pub active: Option<String>,
}

/// Build and track the popup menu (modal; runs with the tray state unborrowed). Returns the chosen command.
pub fn show_menu(api: UiApi, hwnd: HWND, state: &MenuState) -> usize {
    // SAFETY: menu handles are created here and destroyed with the root menu; label buffers outlive AppendMenuW.
    unsafe {
        let menu = (api.create_menu)();
        for (id, label, enabled) in menu_items(&state.status, state.driver, state.paused) {
            // Saved profiles get a submenu just before Configure…; the active one is checked.
            if id == Some(ID_CONFIGURE) && !state.profiles.is_empty() {
                let profiles = (api.create_menu)();
                for (index, name) in state.profiles.iter().enumerate() {
                    let checked = if state.active.as_deref() == Some(name.as_str()) { MF_CHECKED } else { MF_STRING };
                    (api.append_menu)(profiles, checked, ID_PROFILE_BASE + index, ffi::wide(name).as_ptr());
                }
                (api.append_menu)(menu, MF_POPUP, profiles as usize, ffi::wide("Profile").as_ptr());
            }
            let text = ffi::wide(label);
            let flags = match id {
                None => MF_SEPARATOR,
                Some(_) if enabled => MF_STRING,
                Some(_) => MF_STRING | MF_GRAYED,
            };
            (api.append_menu)(menu, flags, id.unwrap_or(0), text.as_ptr());
        }
        let mut point = POINT::default();
        (api.cursor_pos)(&mut point);
        (api.set_foreground)(hwnd);
        let chosen = (api.track_menu)(menu, TPM_RETURNCMD_RIGHT, point.x, point.y, 0, hwnd, std::ptr::null());
        (api.destroy_menu)(menu);
        chosen as usize
    }
}

thread_local! {
    pub static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// Window procedure. The tray borrow is released before the modal menu so re-entrant messages are handled.
///
/// # Safety
/// Called by Windows (or tests) with this thread's window; parameters follow the WNDPROC contract.
pub unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> isize {
    // Shell calls made while handling a message (Shell_NotifyIconW, ShellExecuteW) can deliver messages
    // re-entrantly. Those see the tray borrowed: they get default handling, and status refreshes are re-queued.
    let action = TRAY.with(|tray| {
        tray.try_borrow_mut().ok().map(|mut t| t.as_mut().map(|t| (t.handle(message, wparam, lparam), t.api, t.menu_state())))
    });
    match action {
        Some(Some((Action::Done(result), _, _))) => result,
        Some(Some((Action::Menu, api, state))) => {
            let chosen = show_menu(api, hwnd, &state);
            let destroy = TRAY.with(|tray| tray.try_borrow_mut().ok().is_some_and(|mut t| t.as_mut().is_some_and(|t| t.command(chosen))));
            if destroy {
                (api.destroy_window)(hwnd);
            }
            0
        }
        Some(Some((Action::Destroy, api, _))) => {
            (api.destroy_window)(hwnd);
            0
        }
        Some(Some((Action::Default, api, _))) => (api.def_proc)(hwnd, message, wparam, lparam),
        Some(None) => ffi::DefWindowProcW(hwnd, message, wparam, lparam),
        None => {
            if message == WM_STATUS {
                ffi::PostMessageW(hwnd, message, wparam, lparam);
            }
            ffi::DefWindowProcW(hwnd, message, wparam, lparam)
        }
    }
}
