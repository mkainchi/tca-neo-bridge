//! The HTTP client against the daemon's real API server, and the replies parsed into the shared types.
mod common;
use common::TOKEN;
use neo_common::api::{ConfigReply, Live, Profiles};
use neo_config::api::{self, Api, ApiError, Http};
use neo_daemon::shared::{Links, Shared};
use std::sync::Arc;

fn server() -> (Arc<Shared>, neo_daemon::http::Server) {
    let dir = std::env::temp_dir().join(format!("neo-config-api-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shared = Arc::new(Shared::new(dir.join("config.json"), neo_daemon::config::defaults()));
    let server = neo_daemon::http::Server::start(shared.clone(), TOKEN.into(), "127.0.0.1:0").unwrap();
    (shared, server)
}

#[test]
fn requests_carry_the_exact_host_and_the_token() {
    assert_eq!(
        api::request(4321, "t", "PUT", "/api/preview", Some("{}")),
        "PUT /api/preview HTTP/1.1\r\nHost: 127.0.0.1:4321\r\nX-Neo-Token: t\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
    );
    assert!(api::request(1, "t", "GET", "/api/live", None).ends_with("Content-Length: 0\r\nConnection: close\r\n\r\n"));
}

#[test]
fn replies_map_to_bodies_refusals_and_failures() {
    assert_eq!(api::reply(b"HTTP/1.1 200 OK\r\nA: b\r\n\r\n{\"x\":1}"), Ok("{\"x\":1}".into()));
    assert_eq!(api::reply(b"HTTP/1.1 403 Forbidden\r\n\r\n{}"), Err(ApiError::Forbidden));
    assert_eq!(api::reply(b"HTTP/1.1 400 Bad Request\r\n\r\n{\"error\":\"No.\"}"), Err(ApiError::Rejected("No.".into())));
    assert_eq!(api::reply(b"HTTP/1.1 404 Not Found\r\n\r\nNot found"), Err(ApiError::Rejected("The daemon refused the request.".into())));
    assert_eq!(api::reply(b"HTTP/1.1 400 Bad Request\r\n\r\n{\"other\":1}"), Err(ApiError::Rejected("The daemon refused the request.".into())));
    for broken in [&b"HTTP/1.1 500 Oops\r\n\r\n"[..], b"HTTP/1.1 200 OK", b"garbage\r\n\r\n", b"\xff\r\n\r\n"] {
        assert_eq!(api::reply(broken), Err(ApiError::Unreachable));
    }
}

#[test]
fn the_daemon_answers_with_the_shared_reply_types() {
    let (shared, server) = server();
    let mut client = Http::new(server.port, TOKEN.into());
    let config: ConfigReply = serde_json::from_str(&client.call("GET", "/api/config", None).unwrap()).unwrap();
    assert_eq!((config.config, config.status.as_str(), config.preview, config.links), (neo_daemon::config::defaults(), "Idle", None, None));
    // A running session with live input and a preview.
    *shared.links.lock().unwrap() = Some(Links { source: "tca", hidden: true, tca: true, virtual_neo: true });
    shared.set_preview(Some(neo_daemon::config::defaults())).unwrap();
    *shared.monitor.lock().unwrap() = Some(serde_json::from_value(common::monitor()).unwrap());
    let live: Live = serde_json::from_str(&client.call("GET", "/api/live", None).unwrap()).unwrap();
    assert!(live.preview && live.monitor.is_some());
    assert_eq!(live.links.unwrap().source, "tca");
    let config: ConfigReply = serde_json::from_str(&client.call("GET", "/api/config", None).unwrap()).unwrap();
    assert!(config.preview.is_some() && config.links.is_some_and(|links| links.virtual_neo));
    let profiles: Profiles = serde_json::from_str(&client.call("GET", "/api/profiles", None).unwrap()).unwrap();
    assert_eq!((profiles.profiles.len(), profiles.active), (0, None));
    // The daemon's own refusals.
    assert!(matches!(client.call("PUT", "/api/preview", Some("{")), Err(ApiError::Rejected(message)) if message.starts_with("Invalid profile")));
    assert_eq!(Http::new(server.port, "stale".into()).call("GET", "/api/live", None), Err(ApiError::Forbidden));
    drop(server);
    assert_eq!(client.call("GET", "/api/live", None), Err(ApiError::Unreachable));
}
