//! A background thread that talks to the daemon: requests from the window, and `/api/live` polled while it is
//! visible. Replies go back over a channel and wake the window.
use crate::api::{Api, ApiError};
use neo_common::api::{ConfigReply, Live, Profiles};
use serde::de::DeserializeOwned;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Failed live polls in a row before the daemon counts as gone (about 300 ms at the normal period).
pub const MISSES: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// The saved profile, the daemon's state and the saved profile names.
    Load,
}

/// Why the window must close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fatal {
    /// The daemon no longer accepts this app's token.
    Forbidden,
    /// The daemon stopped answering.
    Lost,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Loaded(Box<ConfigReply>, Profiles),
    Live(Box<Live>),
    Failed(String),
    Fatal(Fatal),
}

fn get<T: DeserializeOwned>(api: &mut dyn Api, path: &str) -> Result<T, ApiError> {
    let body = api.call("GET", path, None)?;
    serde_json::from_str(&body).map_err(|_| ApiError::Rejected("The daemon sent a reply this app does not understand.".into()))
}

fn outcome(error: ApiError) -> Reply {
    match error {
        ApiError::Forbidden => Reply::Fatal(Fatal::Forbidden),
        ApiError::Rejected(message) => Reply::Failed(message),
        ApiError::Unreachable => Reply::Failed("The bridge is not answering.".into()),
    }
}

fn handle(api: &mut dyn Api, request: Request) -> Reply {
    match request {
        Request::Load => get(api, "/api/config").and_then(|config| Ok(Reply::Loaded(Box::new(config), get(api, "/api/profiles")?))).unwrap_or_else(outcome),
    }
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
                    Ok(request) => handle(api.as_mut(), request),
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
