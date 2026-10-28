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
    /// Save the settings draft.
    Save(Box<Config>),
    /// Run the draft in game without saving it.
    Preview(Box<Config>),
    /// Go back to the saved profile in game.
    EndPreview,
    /// Make a saved profile the active one.
    Activate(String),
    /// Save the saved settings under a name (replacing one of that name) and make it active.
    SaveAs(String),
    /// Delete a saved profile (the settings themselves stay).
    DeleteProfile(String),
    /// A profile file (`name`): the daemon checks and upgrades it, then it becomes the saved settings.
    Import { profile: serde_json::Value, name: String },
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
    /// The settings draft is saved (`running`: the emulation applied it), or why not.
    Saved { config: Box<Config>, running: bool },
    SaveFailed(String),
    /// The draft runs in game, or why it does not.
    Previewed,
    PreviewFailed(String),
    /// The saved profile runs in game again, or why it does not.
    PreviewEnded,
    PreviewEndFailed(String),
    /// A profile is now the saved settings (switched to, or imported): the reply, and what to say.
    Adopted { config: Box<Config>, profiles: Option<Profiles>, text: String },
    /// The saved profile names changed (saved as, deleted), and what to say.
    Listed { profiles: Profiles, text: String },
    /// A profile action failed, and what to say.
    ProfileFailed(String),
    Fatal(Fatal),
}

#[derive(Deserialize)]
struct Saved {
    running: bool,
}

/// The daemon's answer to a profile action: the names, the active one, and the saved settings now.
#[derive(Deserialize)]
struct ProfileAction {
    profiles: Vec<String>,
    active: Option<String>,
    config: Config,
    running: bool,
}

/// A profile name in a URL path: everything but letters, digits, `_` and `-` percent-encoded.
pub fn encode(name: &str) -> String {
    name.bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' { char::from(b).to_string() } else { format!("%{b:02X}") }).collect()
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
        Request::Save(config) => match soft(put::<Saved>(api, "/api/config", &config))? {
            Ok(saved) => Reply::Saved { config, running: saved.running },
            Err(message) => Reply::SaveFailed(message),
        },
        Request::Preview(config) => soft(put::<IgnoredAny>(api, "/api/preview", &config))?.map_or_else(Reply::PreviewFailed, |_| Reply::Previewed),
        Request::EndPreview => soft(understood::<IgnoredAny>(api.call("DELETE", "/api/preview", None)))?.map_or_else(Reply::PreviewEndFailed, |_| Reply::PreviewEnded),
        Request::Activate(name) => match soft(understood::<ProfileAction>(api.call("POST", &format!("/api/profiles/{}/activate", encode(&name)), None)))? {
            Ok(done) => {
                let applied = if done.running { " Applied to the running emulation." } else { "" };
                Reply::Adopted { config: Box::new(done.config), profiles: Some(Profiles { profiles: done.profiles, active: done.active }), text: format!("Switched to {name}.{applied}") }
            }
            Err(error) => Reply::ProfileFailed(format!("Not switched: {error}")),
        },
        Request::SaveAs(name) => match soft(put::<ProfileAction>(api, &format!("/api/profiles/{}", encode(&name)), &serde_json::json!({})))? {
            Ok(done) => Reply::Listed { profiles: Profiles { profiles: done.profiles, active: done.active }, text: format!("Saved as {name}.") },
            Err(error) => Reply::ProfileFailed(format!("Not saved: {error}")),
        },
        Request::DeleteProfile(name) => match soft(understood::<ProfileAction>(api.call("DELETE", &format!("/api/profiles/{}", encode(&name)), None)))? {
            Ok(done) => Reply::Listed { profiles: Profiles { profiles: done.profiles, active: done.active }, text: format!("Deleted {name}. The current settings are kept.") },
            Err(error) => Reply::ProfileFailed(format!("Not deleted: {error}")),
        },
        Request::Import { profile, name } => {
            let imported = match soft(put::<Saved>(api, "/api/config", &profile))? {
                Ok(saved) => soft(get::<ConfigReply>(api, "/api/config"))?.map(|reply| (saved, reply)),
                Err(error) => Err(error),
            };
            match imported {
                Ok((saved, reply)) => Reply::Adopted {
                    config: Box::new(reply.config),
                    profiles: None,
                    text: format!("Imported {name}.{} Save it under a name to keep it as a profile.", if saved.running { " Applied to the running emulation." } else { "" }),
                },
                Err(error) => Reply::ProfileFailed(format!("Not imported: {error}")),
            }
        }
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
