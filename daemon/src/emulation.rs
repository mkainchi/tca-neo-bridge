//! Emulation session: HidHide isolation → TCA readers → report pipe → virtual NEO, torn down in reverse.
use crate::device::{DeviceApi, PipeServer, SoftwareDevice};
use crate::driver::Notice;
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

pub const DISCONNECTED: &str = "A TCA device disconnected. Controls were released; reconnect it and start emulation again.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Tca,
    Validation,
}

pub type HidingFactory = Box<dyn Fn() -> Result<Hiding, String> + Send + Sync>;
pub type Notify = Arc<dyn Fn() + Send + Sync>;

pub struct Deps {
    pub hid: HidApi,
    pub device: DeviceApi,
    pub pipe_name: String,
    pub hiding: HidingFactory,
    pub period: Duration,
    pub device_timeout: Duration,
}

/// Owned resources, released in a fixed order: virtual device, pipe (neutral first), readers, HidHide.
#[derive(Default)]
struct Resources {
    hiding: Option<Hiding>,
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

fn run_loop(resources: &Resources, shared: &Shared, period: Duration, stop: &AtomicBool, notify: &Notify) -> Result<(), String> {
    let started = Instant::now();
    let (mut generation, config) = shared.active();
    let mut mapper = Mapper::new(&config);
    let pipe = resources.pipe.as_ref().unwrap();
    while !stop.load(Ordering::SeqCst) {
        let report = match &resources.readers {
            Some((stick, quadrant)) => {
                if stick.failed.load(Ordering::SeqCst) || quadrant.failed.load(Ordering::SeqCst) {
                    return Err(DISCONNECTED.into());
                }
                if shared.generation() != generation {
                    let (latest, config) = shared.active();
                    generation = latest;
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
                let report = mapper.at(started.elapsed()).convert(stick.as_deref(), quadrant.as_deref());
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
        let tca = source == Source::Tca;
        *shared.links.lock().unwrap() = Some(Links { source: if tca { "tca" } else { "validation" }, hidden: tca, tca, virtual_neo: true });
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_stop, period) = (stop.clone(), deps.period);
        let thread = std::thread::spawn(move || {
            let outcome = run_loop(&resources, &shared, period, &thread_stop, &notify);
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
