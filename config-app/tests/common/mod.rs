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
        fake.reply("/api/devices", Ok(devices_reply(json!({}))));
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

/// `GET /api/devices` for the TCA pair (its set and the daemon's default mapping), with `extra` fields replaced.
pub fn devices_reply(extra: serde_json::Value) -> String {
    use neo_daemon::{config, models};
    let devices = [models::tca_quadrant(), models::tca_stick()];
    let mut reply = json!({"devices": devices, "active": ["tca-quadrant", "tca-stick"], "setup": config::TCA_SETUP, "profile": config::tca_profile(), "default": config::tca_profile()});
    reply.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    reply.to_string()
}

pub fn monitor() -> serde_json::Value {
    json!({"roll": 0.1, "pitch": -0.2, "yaw": 0.0, "throttle": 0.5, "camera_lever": null, "levers": [0.2, 0.8], "axes": [32768, 32768, 32768, 32768, 32768, 32768, 32768], "armed": true, "precision": false, "shift": false, "buttons": 0, "pov": 8, "head": null})
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

/// The TCA pair as the window sees it: both present and taking part.
pub fn tca() -> neo_config::mapping::Controllers {
    use neo_daemon::models;
    neo_config::mapping::Controllers { present: vec![models::tca_quadrant(), models::tca_stick()], active: vec!["tca-quadrant".into(), "tca-stick".into()] }
}

/// The default settings with the TCA pair's default mapping, as the window edits them.
pub fn view() -> neo_config::view::View {
    neo_config::view::View::of(&neo_daemon::config::defaults(), neo_daemon::config::TCA_SETUP, &neo_daemon::config::tca_profile())
}

fn device(id: &str, name: &str, pid: u16, buttons: u16, axes: &[&str], hat: bool) -> neo_daemon::models::DeviceInfo {
    use neo_daemon::models::{Axis, DeviceInfo, Layout};
    let axes = axes.iter().map(|axis| Axis { name: axis.to_string(), max: 65535 }).collect();
    DeviceInfo { id: id.into(), name: name.into(), vid: 0x044f, pid, layout: Layout { buttons, axes, hat } }
}

/// A T.16000M stick (16 buttons, x y rz slider, hat) and a TWCS throttle (14 buttons, z rx ry rz slider, hat): a
/// non-TCA set.
pub fn t16000m() -> neo_daemon::models::DeviceInfo {
    device("t16000m", "T.16000M", 0xb10a, 16, &["x", "y", "rz", "slider"], true)
}

pub fn twcs() -> neo_daemon::models::DeviceInfo {
    device("twcs", "TWCS Throttle", 0xb687, 14, &["z", "rx", "ry", "rz", "slider"], true)
}