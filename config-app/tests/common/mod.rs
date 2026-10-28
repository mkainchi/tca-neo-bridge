//! A scripted daemon API (the role the page tests' fakeApi played) and replies built from the real daemon types.
#![allow(dead_code)]
use neo_config::api::{Api, ApiError};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";

#[derive(Default)]
pub struct Script {
    /// (method, path) of every call, in order.
    pub calls: Vec<(String, String)>,
    /// The body of every call that had one, by path.
    pub bodies: Vec<(String, String)>,
    /// Replies by path; a path without one is unreachable. PUTs use "PUT <path>" when one is set.
    pub replies: std::collections::HashMap<String, Result<String, ApiError>>,
    /// Replies for `/api/live` taken in turn before falling back to `replies`.
    pub live: VecDeque<Result<String, ApiError>>,
}

#[derive(Clone, Default)]
pub struct FakeApi(pub Arc<Mutex<Script>>);

impl FakeApi {
    /// A daemon that is idle with the default profile and no saved profiles.
    pub fn idle() -> Self {
        let fake = Self::default();
        fake.reply("/api/config", Ok(config_reply(json!({}))));
        fake.reply("/api/profiles", Ok(json!({"profiles": [], "active": null}).to_string()));
        fake.reply("/api/live", Ok(live_reply(json!({}))));
        fake.reply("/api/labels", Ok(json!({"labels": []}).to_string()));
        fake.reply("PUT /api/config", Ok(json!({"saved": true, "running": false}).to_string()));
        fake.reply("PUT /api/labels", Ok(json!({"saved": 1}).to_string()));
        fake
    }

    pub fn reply(&self, path: &str, reply: Result<String, ApiError>) {
        self.0.lock().unwrap().replies.insert(path.into(), reply);
    }

    pub fn calls(&self) -> Vec<(String, String)> {
        self.0.lock().unwrap().calls.clone()
    }

    pub fn bodies(&self) -> Vec<(String, String)> {
        self.0.lock().unwrap().bodies.clone()
    }

    pub fn count(&self, path: &str) -> usize {
        self.calls().iter().filter(|(_, p)| p == path).count()
    }
}

impl Api for FakeApi {
    fn call(&mut self, method: &str, path: &str, body: Option<&str>) -> Result<String, ApiError> {
        let mut script = self.0.lock().unwrap();
        script.calls.push((method.into(), path.into()));
        script.bodies.extend(body.map(|body| (path.to_string(), body.to_string())));
        if path == "/api/live" {
            if let Some(reply) = script.live.pop_front() {
                return reply;
            }
        }
        let put = format!("{method} {path}");
        script.replies.get(&put).or_else(|| script.replies.get(path)).cloned().unwrap_or(Err(ApiError::Unreachable))
    }
}

/// `GET /api/config` as the daemon sends it, with `extra` fields replaced.
pub fn config_reply(extra: serde_json::Value) -> String {
    let defaults = neo_daemon::config::defaults();
    let mut reply = json!({"config": defaults, "defaults": defaults, "status": "Idle", "links": null, "profile": r"C:\data\config.json", "preview": null, "warnings": []});
    reply.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    reply.to_string()
}

/// `GET /api/live` as the daemon sends it, with `extra` fields replaced.
pub fn live_reply(extra: serde_json::Value) -> String {
    let mut reply = json!({"pressed": [], "levers": null, "monitor": null, "paused": false, "preview": false, "warnings": [], "status": "Idle", "links": null});
    reply.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    reply.to_string()
}

pub fn monitor() -> serde_json::Value {
    json!({"roll": 0.1, "pitch": -0.2, "yaw": 0.0, "slider": 0.5, "levers": [0.2, 0.8], "axes": [32768, 32768, 32768, 32768, 32768, 32768, 32768], "armed": true, "precision": false, "shift": false, "buttons": 0, "pov": 8, "head": null})
}

pub fn wait_until(condition: impl FnMut() -> bool) {
    wait_for(Duration::from_secs(10), condition);
}

/// Wait up to `limit` for `condition` (the real app starts slowly on CI runners).
pub fn wait_for(limit: Duration, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}
