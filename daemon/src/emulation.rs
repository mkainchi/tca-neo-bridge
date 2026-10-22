//! Emulation session: HidHide isolation → controller readers → report pipe → virtual NEO, torn down in reverse.
use crate::device::{DeviceApi, PipeServer, SoftwareDevice};
use crate::driver::Notice;
use crate::headtrack::Listener;
use crate::hidhide::Hiding;
use crate::neo::{self, Event, Mapper};
use crate::profiles;
use crate::shared::{Links, Shared, Status};
use crate::hid::{self, HidApi, Reader};
use crate::models;
use crate::validation;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const RECONNECT_FAILED: &str = "A controller came back but could not be hidden from the game again. Controls were released; start emulation again.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Hotas,
    Validation,
}

pub type HidingFactory = Box<dyn Fn() -> Result<Hiding, String> + Send + Sync>;
pub type Notify = Arc<dyn Fn() + Send + Sync>;
/// Launch-order warnings, checked when emulation starts.
pub type Checks = Box<dyn Fn() -> Vec<String> + Send + Sync>;

pub struct Deps {
    pub hid: HidApi,
    pub device: DeviceApi,
    pub pipe_name: String,
    pub hiding: HidingFactory,
    pub period: Duration,
    pub device_timeout: Duration,
    /// How often to look for a controller that disconnected during emulation.
    pub retry: Duration,
    pub checks: Checks,
}

/// Owned resources, released in a fixed order: virtual device, pipe (neutral first), readers, HidHide; the
/// head-tracking listener goes with them.
#[derive(Default)]
struct Resources {
    hiding: Option<Hiding>,
    head: Option<Listener>,
    readers: Option<Vec<Reader>>,
    pipe: Option<PipeServer>,
    device: Option<SoftwareDevice>,
}

impl Resources {
    fn release(mut self) -> Result<(), String> {
        if let Some(pipe) = &self.pipe {
            pipe.send(neo::neutral());
        }
        drop(self.device.take());
        drop(self.pipe.take());
        drop(self.readers.take());
        self.hiding.take().map_or(Ok(()), |mut hiding| hiding.close())
    }
}

fn acquire(deps: &Deps, source: Source, wanted: &[String], resources: &mut Resources) -> Result<(), String> {
    if source == Source::Hotas {
        let hiding = resources.hiding.insert((deps.hiding)()?);
        hiding.start(&|vid, pid| models::wanted(wanted, vid, pid))?;
        let opened = hid::find(&deps.hid, wanted)?;
        resources.readers = Some(opened.into_iter().map(|o| Reader::spawn(deps.hid, o)).collect());
    }
    resources.pipe = Some(PipeServer::start(deps.device, &deps.pipe_name, deps.period)?);
    resources.device = Some(SoftwareDevice::create(deps.device, deps.device_timeout)?);
    Ok(())
}

pub struct Emulation {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// The next-profile combo: activate the saved profile after the active one, and say which (or why not).
fn next_profile(shared: &Shared) -> Notice {
    let dir = shared.profiles_dir();
    let next = profiles::next(&dir, profiles::active(&dir).as_deref()).ok_or_else(|| "No saved profiles yet: save one on the configuration page.".to_string());
    match next.and_then(|name| shared.activate(&name).map(|_| name)) {
        Ok(name) => Notice { title: "Profile switched", text: format!("Now flying with {name}."), error: false },
        Err(error) => Notice { title: "Profile not switched", text: error, error: true },
    }
}

/// Keep the session's controller pill in step with the readers (the links are set before the loop runs).
fn inputs_link(shared: &Shared, open: usize) {
    shared.links.lock().unwrap().as_mut().expect("session links").inputs = open;
}

/// The session's controllers again, once every one of them is back (a different set keeps waiting).
fn reopen(hid: &HidApi, wanted: &[String], devices: &[models::DeviceInfo]) -> Option<Vec<hid::Opened>> {
    let opened = hid::find(hid, wanted).ok()?;
    let mut found: Vec<&str> = opened.iter().map(|o| o.info.id.as_str()).collect();
    let mut session: Vec<&str> = devices.iter().map(|d| d.id.as_str()).collect();
    found.sort_unstable();
    session.sort_unstable();
    if found == session {
        return Some(opened);
    }
    opened.into_iter().for_each(|o| o.close(hid));
    None
}

fn run_loop(resources: &mut Resources, hid: HidApi, retry: Duration, shared: &Shared, period: Duration, stop: &AtomicBool, notify: &Notify) -> Result<(), String> {
    let started = Instant::now();
    let (mut generation, config) = shared.active();
    let wanted = config.devices.clone();
    let devices: Vec<_> = resources.readers.iter().flatten().map(|r| r.info.clone()).collect();
    let mut mapper = Mapper::new(&config, &devices);
    let mut head_timeout = Duration::from_millis(u64::from(config.head_tracking.timeout_ms));
    let Resources { hiding, head, readers, pipe, .. } = resources;
    let pipe = pipe.as_ref().unwrap();
    // While a controller is away: when to look for it next. The virtual NEO stays, so the game keeps its controller.
    let mut search: Option<Instant> = None;
    while !stop.load(Ordering::SeqCst) {
        if readers.as_ref().is_some_and(|list| list.iter().any(|r| r.failed.load(Ordering::SeqCst))) {
            *readers = None;
            mapper.convert(&[]); // releases everything, re-arms safe start and re-primes switch states
            shared.pressed.lock().unwrap().clear();
            *shared.levers.lock().unwrap() = None;
            *shared.monitor.lock().unwrap() = None;
            shared.axes.lock().unwrap().clear();
            inputs_link(shared, 0);
            shared.notice(Notice { title: "Controller disconnected", text: "Controls released. Waiting for it to return. Avoid unpowered USB hubs.".into(), error: true });
            notify();
            search = Some(Instant::now() + retry);
        }
        if let Some(at) = search {
            if Instant::now() >= at {
                search = match reopen(&hid, &wanted, &devices) {
                    Some(opened) => {
                        // A replugged device may be a new instance: hide it before the game can see it.
                        hiding.as_mut().unwrap().rehide(&|vid, pid| models::wanted(&wanted, vid, pid)).map_err(|_| RECONNECT_FAILED.to_string())?;
                        *readers = Some(opened.into_iter().map(|o| Reader::spawn(hid, o)).collect());
                        inputs_link(shared, devices.len());
                        shared.notice(Notice { title: "Controller reconnected", text: "Back in control. With safe start on, move the throttle to centre once.".into(), error: false });
                        notify();
                        None
                    }
                    None => Some(Instant::now() + retry),
                };
            }
            if search.is_some() {
                pipe.send(neo::neutral());
                std::thread::sleep(period);
                continue;
            }
        }
        let report = match readers.as_ref() {
            Some(list) => {
                if shared.generation() != generation {
                    let (latest, config) = shared.active();
                    generation = latest;
                    head_timeout = Duration::from_millis(u64::from(config.head_tracking.timeout_ms));
                    mapper.reconfigure(&config);
                }
                let inputs: Vec<_> = list.iter().map(Reader::snapshot).collect();
                // The hat is reported as its cardinal components ("…/hat-up", "…/hat-right"), matching the mappable hat sources.
                *shared.pressed.lock().unwrap() = if inputs.iter().all(Option::is_some) { neo::pressed(&devices, &inputs) } else { Vec::new() };
                *shared.levers.lock().unwrap() = mapper.levers(&inputs);
                *shared.axes.lock().unwrap() = neo::axes(&devices, &inputs);
                let sample = head.as_ref().and_then(|listener| listener.sample(head_timeout));
                let report = mapper.at(started.elapsed()).head(sample).convert(&inputs);
                *shared.monitor.lock().unwrap() = mapper.monitor();
                for event in mapper.take_events() {
                    match event {
                        Event::TogglePause => shared.set_paused(!shared.paused()),
                        Event::NextProfile => shared.notice(next_profile(shared)),
                    }
                    notify();
                }
                if shared.paused() {
                    neo::neutral()
                } else {
                    report
                }
            }
            None => validation::report(started.elapsed().as_secs_f64()),
        };
        pipe.send(report);
        std::thread::sleep(period);
    }
    Ok(())
}

impl Emulation {
    pub fn start(deps: &Deps, shared: Arc<Shared>, source: Source, notify: Notify) -> Result<Self, String> {
        let mut resources = Resources::default();
        if let Err(error) = acquire(deps, source, &shared.config().1.devices, &mut resources) {
            return Err(resources.release().err().unwrap_or(error));
        }
        // Head tracking starts with the session; a busy port is reported, not fatal.
        let tracking = shared.active().1.head_tracking;
        if tracking.enabled {
            match Listener::start(tracking.port) {
                Ok(listener) => resources.head = Some(listener),
                Err(error) => shared.notice(Notice { title: "Head tracking off", text: error, error: true }),
            }
        }
        let hotas = source == Source::Hotas;
        let devices: Vec<_> = resources.readers.iter().flatten().map(|r| r.info.clone()).collect();
        *shared.links.lock().unwrap() = Some(Links { source: if hotas { "hotas" } else { "validation" }, hidden: hotas, inputs: devices.len(), virtual_neo: true });
        *shared.devices.lock().unwrap() = hotas.then_some(devices);
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_stop, period, hid, retry) = (stop.clone(), deps.period, deps.hid, deps.retry);
        let thread = std::thread::spawn(move || {
            let outcome = run_loop(&mut resources, hid, retry, &shared, period, &thread_stop, &notify);
            shared.pressed.lock().unwrap().clear();
            *shared.levers.lock().unwrap() = None;
            shared.axes.lock().unwrap().clear();
            *shared.devices.lock().unwrap() = None;
            *shared.monitor.lock().unwrap() = None;
            shared.set_paused(false);
            let _ = shared.set_preview(None);
            let released = resources.release();
            *shared.links.lock().unwrap() = None;
            shared.set_status(match outcome.and(released) {
                Ok(()) => Status::Idle,
                Err(error) => Status::Error(error),
            });
            notify();
        });
        Ok(Self { stop, thread: Some(thread) })
    }
}

impl Drop for Emulation {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}

/// Tray-facing commands; start/stop run off the UI thread and report through `notify`.
pub struct Controller {
    pub shared: Arc<Shared>,
    deps: Arc<Deps>,
    session: Arc<Mutex<Option<Emulation>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    notify: Notify,
    source: Source,
}

impl Controller {
    pub fn new(shared: Arc<Shared>, deps: Deps, source: Source, notify: Notify) -> Self {
        Self { shared, deps: Arc::new(deps), session: Arc::new(Mutex::new(None)), worker: Mutex::new(None), notify, source }
    }

    fn spawn(&self, work: impl FnOnce() + Send + 'static) {
        self.wait();
        *self.worker.lock().unwrap() = Some(std::thread::spawn(work));
    }

    /// Wait for a pending start/stop to finish.
    pub fn wait(&self) {
        if let Some(worker) = self.worker.lock().unwrap().take() {
            worker.join().unwrap();
        }
    }

    pub fn start(&self) {
        if self.shared.status().running() {
            return;
        }
        self.shared.set_status(Status::Starting);
        (self.notify)();
        let (deps, shared, session, notify, source) = (self.deps.clone(), self.shared.clone(), self.session.clone(), self.notify.clone(), self.source);
        self.spawn(move || {
            let warnings = (deps.checks)();
            for warning in &warnings {
                shared.notice(Notice { title: "Check Steam", text: warning.clone(), error: false });
            }
            *shared.warnings.lock().unwrap() = warnings;
            let mut slot = session.lock().unwrap();
            drop(slot.take());
            match Emulation::start(&deps, shared.clone(), source, notify.clone()) {
                Ok(emulation) => {
                    *slot = Some(emulation);
                    shared.set_status(Status::Emulating);
                }
                Err(error) => shared.set_status(Status::Error(error)),
            }
            notify();
        });
    }

    pub fn stop(&self) {
        if self.shared.status() != Status::Emulating {
            return;
        }
        self.shared.set_status(Status::Stopping);
        (self.notify)();
        let session = self.session.clone();
        self.spawn(move || drop(session.lock().unwrap().take()));
    }

    /// Synchronous stop used by Quit.
    pub fn shutdown(&self) {
        self.wait();
        drop(self.session.lock().unwrap().take());
    }
}
