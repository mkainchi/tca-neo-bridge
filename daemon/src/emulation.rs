//! Emulation session: HidHide isolation → TCA readers → report pipe → virtual NEO, torn down in reverse.
use crate::device::{DeviceApi, PipeServer, SoftwareDevice};
use crate::driver::Notice;
use crate::headtrack::Listener;
use crate::hidhide::Hiding;
use crate::neo::{self, Event, Mapper};
use crate::profiles;
use crate::shared::{Links, Shared, Status};
use crate::tca::{self, HidApi, Reader};
use crate::validation;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const RECONNECT_FAILED: &str = "The TCA came back but could not be hidden from the game again. Controls were released; start emulation again.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Tca,
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
    /// How often to look for a TCA that disconnected during emulation.
    pub retry: Duration,
    pub checks: Checks,
}

/// Owned resources, released in a fixed order: virtual device, pipe (neutral first), readers, HidHide; the
/// head-tracking listener goes with them.
#[derive(Default)]
struct Resources {
    hiding: Option<Hiding>,
    head: Option<Listener>,
    readers: Option<(Reader, Reader)>,
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

fn acquire(deps: &Deps, source: Source, resources: &mut Resources) -> Result<(), String> {
    if source == Source::Tca {
        let hiding = resources.hiding.insert((deps.hiding)()?);
        hiding.start()?;
        let (stick, quadrant) = tca::find_pair(&deps.hid)?;
        resources.readers = Some((Reader::spawn(deps.hid, stick), Reader::spawn(deps.hid, quadrant)));
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

/// Keep the session's TCA link pill in step with the readers (the links are set before the loop runs).
fn tca_link(shared: &Shared, open: bool) {
    shared.links.lock().unwrap().as_mut().expect("session links").tca = open;
}

fn run_loop(resources: &mut Resources, hid: HidApi, retry: Duration, shared: &Shared, period: Duration, stop: &AtomicBool, notify: &Notify) -> Result<(), String> {
    let started = Instant::now();
    let (mut generation, config) = shared.active();
    let mut mapper = Mapper::new(&config);
    let mut head_timeout = Duration::from_millis(u64::from(config.head_tracking.timeout_ms));
    let Resources { hiding, head, readers, pipe, .. } = resources;
    let pipe = pipe.as_ref().unwrap();
    // While the TCA is away: when to look for it next. The virtual NEO stays, so the game keeps its controller.
    let mut search: Option<Instant> = None;
    while !stop.load(Ordering::SeqCst) {
        if readers.as_ref().is_some_and(|(stick, quadrant)| stick.failed.load(Ordering::SeqCst) || quadrant.failed.load(Ordering::SeqCst)) {
            *readers = None;
            mapper.convert(None, None); // releases everything, re-arms safe start and re-primes switch states
            shared.pressed.lock().unwrap().clear();
            *shared.levers.lock().unwrap() = None;
            *shared.monitor.lock().unwrap() = None;
            tca_link(shared, false);
            shared.notice(Notice { title: "TCA disconnected", text: "Controls released. Waiting for it to return. Avoid unpowered USB hubs.".into(), error: true });
            notify();
            search = Some(Instant::now() + retry);
        }
        if let Some(at) = search {
            if Instant::now() >= at {
                search = match tca::find_pair(&hid) {
                    Ok((stick, quadrant)) => {
                        // A replugged device may be a new instance: hide it before the game can see it.
                        hiding.as_mut().unwrap().rehide().map_err(|_| RECONNECT_FAILED.to_string())?;
                        *readers = Some((Reader::spawn(hid, stick), Reader::spawn(hid, quadrant)));
                        tca_link(shared, true);
                        shared.notice(Notice { title: "TCA reconnected", text: "Back in control. With safe start on, move the throttle to centre once.".into(), error: false });
                        notify();
                        None
                    }
                    Err(_) => Some(Instant::now() + retry),
                };
            }
            if search.is_some() {
                pipe.send(neo::neutral());
                std::thread::sleep(period);
                continue;
            }
        }
        let report = match readers.as_ref() {
            Some((stick, quadrant)) => {
                if shared.generation() != generation {
                    let (latest, config) = shared.active();
                    generation = latest;
                    head_timeout = Duration::from_millis(u64::from(config.head_tracking.timeout_ms));
                    mapper.reconfigure(&config);
                }
                let (stick, quadrant) = (stick.snapshot(), quadrant.snapshot());
                *shared.pressed.lock().unwrap() = match (&stick, &quadrant) {
                    // Readers only publish complete reports of their role's length.
                    // The hat is reported as its cardinal components ("hat-up", "hat-right"), matching the mappable hat sources.
                    (Some(s), Some(q)) => neo::pressed(s, q).into_iter().chain(neo::hat(s).into_iter().flat_map(|d| d.split('-')).map(|d| format!("hat-{d}"))).collect(),
                    _ => Vec::new(),
                };
                *shared.levers.lock().unwrap() = quadrant.as_deref().map(neo::levers);
                let sample = head.as_ref().and_then(|listener| listener.sample(head_timeout));
                let report = mapper.at(started.elapsed()).head(sample).convert(stick.as_deref(), quadrant.as_deref());
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
        if let Err(error) = acquire(deps, source, &mut resources) {
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
        let tca = source == Source::Tca;
        *shared.links.lock().unwrap() = Some(Links { source: if tca { "tca" } else { "validation" }, hidden: tca, tca, virtual_neo: true });
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_stop, period, hid, retry) = (stop.clone(), deps.period, deps.hid, deps.retry);
        let thread = std::thread::spawn(move || {
            let outcome = run_loop(&mut resources, hid, retry, &shared, period, &thread_stop, &notify);
            shared.pressed.lock().unwrap().clear();
            *shared.levers.lock().unwrap() = None;
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
