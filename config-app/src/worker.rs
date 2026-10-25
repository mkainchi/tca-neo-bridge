//! A background thread that talks to the daemon: requests from the window, and `/api/live` polled while it is
//! visible. Replies go back over a channel and wake the window.
use crate::api::{Api, ApiError};
use neo_common::api::{ConfigReply, Live, Profiles};
use neo_common::config::Config;
use neo_common::labels::Entry;
use serde::de::{DeserializeOwned, IgnoredAny};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Failed live polls in a row before the daemon counts as gone (about 300 ms at the normal period).
pub const MISSES: u32 = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// The saved profile, the daemon's state, the saved profile names and the correction log.
    Load,
    /// Save the profile with a recorded mapping, then the correction log with its new entry.
    Record { config: Box<Config>, labels: Vec<Entry> },
    /// Save the profile with a new hat mode.
    SetHat(Box<Config>),
    /// Save the correction log with an entry removed.
    Forget(Vec<Entry>),
}

/// Why the window must close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fatal {
    /// The daemon no longer accepts this app's token.
    Forbidden,
    /// The daemon stopped answering.
    Lost,
}

/// What the window opens with: the profile and state, and the parts that may fail on their own.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub config: ConfigReply,
    pub profiles: Result<Profiles, String>,
    pub labels: Result<Vec<Entry>, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Loaded(Box<Loaded>),
    Live(Box<Live>),
    /// The window could not load.
    Failed(String),
    /// The mapping is saved (`running`: the emulation applied it); the log, saved or why not.
    Recorded { config: Box<Config>, running: bool, log: Result<Vec<Entry>, String> },
    HatSet { config: Box<Config>, running: bool },
    /// A profile change the daemon refused, and why.
    NotSaved(String),
    /// The correction log after a removal, or why it was not saved.
    Forgot(Result<Vec<Entry>, String>),
    Fatal(Fatal),
}

#[derive(Deserialize)]
struct Saved {
    running: bool,
}

#[derive(Deserialize)]
struct LabelsReply {
    labels: Vec<Entry>,
}

fn understood<T: DeserializeOwned>(body: Result<String, ApiError>) -> Result<T, ApiError> {
    serde_json::from_str(&body?).map_err(|_| ApiError::Rejected("The daemon sent a reply this app does not understand.".into()))
}

fn get<T: DeserializeOwned>(api: &mut dyn Api, path: &str) -> Result<T, ApiError> {
    understood(api.call("GET", path, None))
}

fn put<T: DeserializeOwned>(api: &mut dyn Api, path: &str, body: &impl Serialize) -> Result<T, ApiError> {
    understood(api.call("PUT", path, Some(&serde_json::to_string(body).expect("serializes"))))
}

/// A failure the window can show, or the end of the window when the token is stale.
fn soft<T>(result: Result<T, ApiError>) -> Result<Result<T, String>, Fatal> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(ApiError::Forbidden) => Err(Fatal::Forbidden),
        Err(ApiError::Rejected(message)) => Ok(Err(message)),
        Err(ApiError::Unreachable) => Ok(Err("The bridge is not answering.".into())),
    }
}

fn handle(api: &mut dyn Api, request: Request) -> Result<Reply, Fatal> {
    Ok(match request {
        Request::Load => match soft(get::<ConfigReply>(api, "/api/config"))? {
            Err(message) => Reply::Failed(message),
            Ok(config) => {
                let profiles = soft(get(api, "/api/profiles"))?;
                let labels = soft(get::<LabelsReply>(api, "/api/labels"))?.map(|reply| reply.labels);
                Reply::Loaded(Box::new(Loaded { config, profiles, labels }))
            }
        },
        Request::Record { config, labels } => match soft(put::<Saved>(api, "/api/config", &config))? {
            Err(message) => Reply::NotSaved(message),
            Ok(saved) => Reply::Recorded { log: soft(put::<IgnoredAny>(api, "/api/labels", &labels))?.map(|_| labels), config, running: saved.running },
        },
        Request::SetHat(config) => match soft(put::<Saved>(api, "/api/config", &config))? {
            Err(message) => Reply::NotSaved(message),
            Ok(saved) => Reply::HatSet { config, running: saved.running },
        },
        Request::Forget(labels) => Reply::Forgot(soft(put::<IgnoredAny>(api, "/api/labels", &labels))?.map(|_| labels)),
    })
}
pub struct Worker {
    requests: Option<Sender<Request>>,
    pub replies: Receiver<Reply>,
    visible: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// `wake` runs after every reply (the window repaints to read it).
    pub fn start(mut api: Box<dyn Api>, period: Duration, wake: Box<dyn Fn() + Send>) -> Self {
        let (requests, inbox) = mpsc::channel::<Request>();
        let (outbox, replies) = mpsc::channel();
        let visible = Arc::new(AtomicBool::new(true));
        let polling = visible.clone();
        let thread = std::thread::spawn(move || {
            let mut misses = 0;
            loop {
                let reply = match inbox.recv_timeout(period) {
                    Ok(request) => handle(api.as_mut(), request).unwrap_or_else(Reply::Fatal),
                    Err(RecvTimeoutError::Timeout) if polling.load(Ordering::SeqCst) => match get::<Live>(api.as_mut(), "/api/live") {
                        Ok(live) => {
                            misses = 0;
                            Reply::Live(Box::new(live))
                        }
                        Err(ApiError::Forbidden) => Reply::Fatal(Fatal::Forbidden),
                        Err(_) if misses + 1 < MISSES => {
                            misses += 1;
                            continue;
                        }
                        Err(_) => Reply::Fatal(Fatal::Lost),
                    },
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                let fatal = matches!(reply, Reply::Fatal(_));
                // The window holds the receiver until this thread has ended.
                let _ = outbox.send(reply);
                wake();
                if fatal {
                    break;
                }
            }
        });
        Self { requests: Some(requests), replies, visible, thread: Some(thread) }
    }

    pub fn send(&self, request: Request) {
        let _ = self.requests.as_ref().expect("worker running").send(request);
    }

    /// Live input is polled only while the window can be seen.
    pub fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::SeqCst);
    }
}

impl Drop for Worker {
    /// Requests already sent are finished before the thread ends.
    fn drop(&mut self) {
        self.requests.take();
        let _ = self.thread.take().expect("worker thread").join();
    }
}
