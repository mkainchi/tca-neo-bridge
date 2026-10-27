//! TCA-NEO bridge configuration app: a native window over the daemon's loopback API, started from the tray.
pub mod api;
pub mod diagrams;
pub mod launch;
pub mod map;
pub mod mapping;
pub mod previews;
pub mod settings;
pub mod state;
pub mod theme;
pub mod ui;
pub mod worker;
pub use neo_common::ffi;

use crate::api::{Api, Http};
use crate::launch::{Signals, SystemApi};
use crate::state::State;
use crate::worker::{Request, Worker};
use eframe::egui::{self, ViewportCommand};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

pub const TITLE: &str = "TCA-NEO bridge";
/// `/api/live` while the window is visible.
pub const POLL: Duration = Duration::from_millis(100);
/// The approved design's desktop size, and the smallest window it lays out without scrolling.
pub const SIZE: [f32; 2] = [1600.0, 900.0];
pub const MIN_SIZE: [f32; 2] = [1100.0, 700.0];
pub const NOT_FROM_TRAY: &str = "Open the configuration from the TCA-NEO bridge tray icon (Configure\u{2026}).";

pub struct App {
    pub state: State,
    worker: Worker,
    signals: Arc<Signals>,
    /// The clock of the press timeline.
    started: std::time::Instant,
    /// Requests made this frame (by replies, the close button and the controls), sent after drawing.
    pending: Vec<Request>,
}

impl App {
    pub fn new(ctx: &egui::Context, api: Box<dyn Api>, signals: Arc<Signals>, period: Duration, fonts: &std::path::Path) -> Self {
        // The profile is asked for first, so it never waits on fonts or drawing.
        let wake = ctx.clone();
        let worker = Worker::start(api, period, Box::new(move || wake.request_repaint()));
        worker.send(Request::Load);
        theme::install(ctx, fonts);
        diagrams::install(ctx, fonts);
        Self { state: State::default(), worker, signals, started: std::time::Instant::now(), pending: Vec::new() }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.signals.show.swap(false, Ordering::SeqCst) {
            for command in [ViewportCommand::Visible(true), ViewportCommand::Minimized(false), ViewportCommand::Focus] {
                ctx.send_viewport_cmd(command);
            }
        }
        let now = self.started.elapsed().as_secs_f64() * 1000.0;
        self.state.now = now;
        while let Ok(reply) = self.worker.replies.try_recv() {
            let follow = self.state.apply(reply, now);
            self.pending.extend(follow);
        }
        // Closing the window ends a preview: the game goes back to the saved profile.
        if ctx.input(|input| input.viewport().close_requested()) {
            let end = self.state.settings.set_preview(false);
            self.pending.extend(end);
        }
        if self.signals.quit.load(Ordering::SeqCst) || self.state.closing {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        self.worker.set_visible(!ctx.input(|input| input.viewport().minimized.unwrap_or(false)));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let made = ui::shell(ui, &mut self.state);
        self.pending.extend(made);
        for request in self.pending.drain(..) {
            self.worker.send(request);
        }
    }
}

/// The window: runs until closed; false if it could not be created.
pub type Window = fn(eframe::NativeOptions, Box<dyn FnOnce(&egui::Context) -> App>) -> bool;

pub fn options() -> eframe::NativeOptions {
    // An empty icon leaves the title bar to Windows rather than showing egui's logo.
    let icon = theme::window_icon(theme::ICON).unwrap_or_default();
    let viewport = egui::ViewportBuilder::default().with_title(TITLE).with_inner_size(SIZE).with_min_inner_size(MIN_SIZE).with_icon(icon);
    // Direct3D 12 only: present on every supported Windows, and on machines without a GPU driver (VMs, remote
    // desktops, CI) through its WARP software adapter.
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
    let mut options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    options.wgpu_options.wgpu_setup = eframe::egui_wgpu::WgpuSetup::CreateNew(setup);
    options
}

pub fn native(options: eframe::NativeOptions, app: Box<dyn FnOnce(&egui::Context) -> App>) -> bool {
    eframe::run_native(TITLE, options, Box::new(|cc| Ok(Box::new(app(&cc.egui_ctx))))).is_ok()
}

pub struct Apis {
    pub system: SystemApi,
    /// The daemon's handshake line.
    pub stdin: fn() -> String,
    pub window: Window,
    pub fonts: PathBuf,
}

impl Apis {
    pub fn real() -> Self {
        let windows = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or(r"C:\Windows".into()));
        Self { system: SystemApi::real(), stdin: launch::read_stdin, window: native, fonts: windows.join("Fonts") }
    }
}

/// Exit codes: 0 closed normally, 2 not started by the daemon, 3 already open, 4 no window.
pub fn run(apis: Apis) -> i32 {
    let Some(handshake) = launch::handshake(&(apis.stdin)()) else {
        launch::tell(&apis.system, NOT_FROM_TRAY);
        return 2;
    };
    let name = ffi::wide(launch::MUTEX);
    // SAFETY: NUL-terminated name.
    let mutex = unsafe { (apis.system.create_mutex)(std::ptr::null(), 1, name.as_ptr()) };
    // SAFETY: reads this thread's last error immediately after CreateMutexW.
    if mutex == 0 || unsafe { (apis.system.last_error)() } == ffi::ERROR_ALREADY_EXISTS {
        // SAFETY: closing a null handle is harmless; otherwise it is ours.
        unsafe { (apis.system.close)(mutex) };
        return 3;
    }
    let events = launch::events(&apis.system);
    let signals = Arc::new(Signals::default());
    let (system, fonts) = (apis.system, apis.fonts);
    let api = Box::new(Http::new(handshake.port, handshake.token));
    let shown = (apis.window)(
        options(),
        Box::new(move |ctx| {
            if let Some(events) = events {
                let wake = ctx.clone();
                launch::watch(system, events, signals.clone(), Box::new(move || wake.request_repaint()));
            }
            App::new(ctx, api, signals, POLL, &fonts)
        }),
    );
    // SAFETY: mutex handle from CreateMutexW; the event handles end with the process.
    unsafe { (apis.system.close)(mutex) };
    if shown {
        0
    } else {
        4
    }
}
