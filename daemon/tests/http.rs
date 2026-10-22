use neo_daemon::config;
use neo_daemon::http::{self, Request, Server};
use neo_daemon::shared::{Links, Shared, Status};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;

fn shared(name: &str) -> Arc<Shared> {
    let dir: PathBuf = std::env::temp_dir().join(format!("neo-http-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Arc::new(Shared::new(dir.join("config.json"), config::defaults()))
}

fn request(method: &str, path: &str, token: &str, body: &str) -> Request {
    Request { method: method.into(), path: path.into(), host: "127.0.0.1:8000".into(), token: token.into(), body: body.into() }
}

fn json(body: &str) -> serde_json::Value {
    serde_json::from_str(body).unwrap()
}

#[test]
fn parse_requests_and_lengths() {
    let raw = b"PUT /api/config?x=1 HTTP/1.1\r\nHost: 127.0.0.1:9\r\nX-Neo-Token: abc\r\nContent-Length: 2\r\nAccept: */*\r\n\r\n{}";
    let parsed = http::parse(raw).unwrap();
    assert_eq!(parsed, Request { method: "PUT".into(), path: "/api/config".into(), host: "127.0.0.1:9".into(), token: "abc".into(), body: "{}".into() });
    assert_eq!(http::expected_length(raw), Some(raw.len()));
    assert_eq!(http::expected_length(b"GET / HTTP/1.1\r\n\r\n"), Some(18));
    assert_eq!(http::expected_length(b"GET / HTTP/1.1\r\n"), None);
    assert_eq!(http::expected_length(b"GET / HTTP/1.1\r\nContent-Length: x\r\n\r\n"), None);
    for bad in [
        &b"GET / HTTP/1.1\r\n"[..],
        b"\xff\xfe / HTTP/1.1\r\n\r\n",
        b"GET\r\n\r\n",
        b"GET / HTTP/1.1\r\nbroken\r\n\r\n",
        b"GET / HTTP/1.1\r\nContent-Length: nan\r\n\r\n",
        b"GET / HTTP/1.1\r\nContent-Length: 5\r\n\r\nab",
        b"GET / HTTP/1.1\r\nContent-Length: 1\r\n\r\n\xff",
    ] {
        assert_eq!(http::parse(bad), None);
    }
}

#[test]
fn routes_assets_tokens_and_host_checks() {
    let shared = shared("routes");
    let get = |path: &str, token: &str| http::handle(&request("GET", path, token, ""), &shared, 8000, "secret");
    assert_eq!(get("/", "").body, http::INDEX);
    assert_eq!(get("/app.js", "").content_type, "text/javascript; charset=utf-8");
    assert_eq!(get("/style.css", "").body, http::STYLE);
    assert_eq!(get("/missing", "").status, 404);
    for (path, _, body) in http::ASSETS {
        let response = get(path, "");
        assert_eq!((response.status, response.body.as_str()), (200, body), "{path}");
    }
    assert_eq!(get("/device/sidestick-right.svg", "").content_type, "image/svg+xml");
    assert!(get("/device/sidestick-left.svg", "").body.contains("left-hand setup"));
    assert!(get("/device/neo.svg", "").body.contains("T.Flight HOTAS NEO"));
    assert!(get("/device/quadrant.svg", "").body.contains("TCA A320 quadrant"));
    assert_eq!(http::handle(&request("POST", "/device/neo.svg", "", ""), &shared, 8000, "secret").status, 404);
    assert_eq!(get("/api/config", "wrong").status, 403);
    assert_eq!(get("/api/nothing", "secret").status, 404);
    let mut foreign = request("GET", "/", "", "");
    foreign.host = "evil.example:8000".into();
    assert_eq!(http::handle(&foreign, &shared, 8000, "secret").status, 403);
    let config = get("/api/config", "secret");
    assert_eq!(config.status, 200);
    let value = json(&config.body);
    assert_eq!(value["status"], "Idle");
    assert_eq!(value["links"], serde_json::Value::Null);
    assert!(value["profile"].as_str().unwrap().ends_with("config.json"));
    assert_eq!(value["config"], value["defaults"]);
    shared.pressed.lock().unwrap().push("s5".into());
    shared.set_status(Status::Emulating);
    assert_eq!(
        json(&get("/api/live", "secret").body),
        serde_json::json!({"pressed": ["s5"], "levers": null, "monitor": null, "paused": false, "preview": false, "warnings": [], "status": "Emulating", "links": null})
    );
    shared.set_paused(true);
    let mut mapper = neo_daemon::neo::Mapper::new(&config::defaults());
    mapper.convert(Some(&[1u8; 64]), Some(&[1u8; 44]));
    *shared.monitor.lock().unwrap() = mapper.monitor();
    let live = json(&get("/api/live", "secret").body);
    assert_eq!((live["paused"].clone(), live["monitor"]["armed"].clone(), live["monitor"]["axes"].as_array().unwrap().len()), (true.into(), false.into(), 7));
    *shared.links.lock().unwrap() = Some(Links { source: "tca", hidden: true, tca: true, virtual_neo: true });
    assert_eq!(json(&get("/api/live", "secret").body)["links"], serde_json::json!({"source": "tca", "hidden": true, "tca": true, "virtual_neo": true}));
    *shared.levers.lock().unwrap() = Some([0.25, 1.0]);
    assert_eq!(json(&get("/api/live", "secret").body)["levers"], serde_json::json!([0.25, 1.0]));
}

#[test]
fn saving_validates_persists_and_hot_reloads() {
    let shared = shared("save");
    let mut cfg = config::defaults();
    cfg.buttons.insert("q9".into(), vec![15]);
    let put = |body: &str| http::handle(&request("PUT", "/api/config", "t", body), &shared, 8000, "t");
    let saved = put(&config::to_json(&cfg));
    assert_eq!((saved.status, json(&saved.body)), (200, serde_json::json!({"saved": true, "running": false})));
    assert_eq!(shared.config(), (1, cfg.clone()));
    assert_eq!(config::load(&shared.config_path).unwrap(), cfg);
    shared.set_status(Status::Emulating);
    assert_eq!(json(&put(&config::to_json(&cfg)).body)["running"], true);
    let rejected = put(&config::to_json(&cfg).replacen("0.04", "1.5", 1));
    assert_eq!(rejected.status, 400);
    assert!(json(&rejected.body)["error"].as_str().unwrap().contains("Roll deadzone"));
    assert_eq!(shared.generation(), 2);
}

#[test]
fn profiles_are_saved_switched_and_deleted_by_name() {
    let shared = shared("profiles");
    let call = |method: &str, path: &str| http::handle(&request(method, path, "t", ""), &shared, 8000, "t");
    assert_eq!(json(&call("GET", "/api/profiles").body), serde_json::json!({"profiles": [], "active": null}));
    let mut attack = config::defaults();
    attack.axes.yaw.curve = 0.5;
    shared.save(attack.clone()).unwrap();
    let saved = json(&call("PUT", "/api/profiles/Ground%20attack").body);
    assert_eq!((saved["profiles"].clone(), saved["active"].clone()), (serde_json::json!(["Ground attack"]), "Ground attack".into()));
    shared.save(config::defaults()).unwrap();
    call("PUT", "/api/profiles/Cruise");
    let switched = call("POST", "/api/profiles/Ground%20attack/activate");
    assert_eq!(switched.status, 200);
    let switched = json(&switched.body);
    assert_eq!((switched["active"].clone(), switched["config"]["axes"]["yaw"]["curve"].clone(), switched["running"].clone()), ("Ground attack".into(), 0.5.into(), false.into()));
    assert_eq!(shared.config().1, attack);
    // Deleting the active profile clears it; deleting another keeps it.
    call("POST", "/api/profiles/Cruise/activate");
    let deleted = json(&call("DELETE", "/api/profiles/Ground%20attack").body);
    assert_eq!((deleted["profiles"].clone(), deleted["active"].clone()), (serde_json::json!(["Cruise"]), "Cruise".into()));
    assert_eq!(json(&call("DELETE", "/api/profiles/Cruise").body)["active"], serde_json::Value::Null);
    // Errors and unknown routes.
    for (method, path, status, message) in [
        ("PUT", "/api/profiles/..%2Fevil", 400, "Profile names"),
        ("POST", "/api/profiles/Missing/activate", 400, "no profile named Missing"),
        ("DELETE", "/api/profiles/Missing", 400, "no profile named Missing"),
        ("PUT", "/api/profiles/%ZZ", 400, "Unreadable profile name"),
        ("GET", "/api/profiles/Cruise", 404, "Unknown request"),
        ("POST", "/api/profiles/Cruise/rename", 404, "Unknown request"),
    ] {
        let response = call(method, path);
        assert_eq!(response.status, status, "{method} {path}");
        assert!(json(&response.body)["error"].as_str().unwrap().contains(message), "{method} {path}");
    }
}

#[test]
fn percent_decoding() {
    assert_eq!(http::decode("Ground%20attack"), Some("Ground attack".into()));
    assert_eq!(http::decode("plain"), Some("plain".into()));
    assert_eq!(http::decode("%e2%9c%88"), Some("✈".into()));
    for bad in ["%", "%2", "%zz", "%ff", "%e2%9c", "%aé"] {
        assert_eq!(http::decode(bad), None, "{bad}");
    }
}

#[test]
fn previews_run_unsaved_settings_until_cleared_or_saved() {
    let shared = shared("preview");
    let call = |method: &str, path: &str, body: &str| http::handle(&request(method, path, "t", body), &shared, 8000, "t");
    assert_eq!(json(&call("GET", "/api/config", "").body)["preview"], serde_json::Value::Null);
    let mut draft = config::defaults();
    draft.axes.pitch.curve = 0.3;
    let started = call("PUT", "/api/preview", &config::to_json(&draft));
    assert_eq!((started.status, json(&started.body)), (200, serde_json::json!({"preview": true, "running": false})));
    assert_eq!((shared.active().1, shared.config().1), (draft.clone(), config::defaults()));
    assert!(!shared.config_path.exists()); // never saved
    let page = json(&call("GET", "/api/config", "").body);
    assert_eq!((page["preview"]["axes"]["pitch"]["curve"].clone(), json(&call("GET", "/api/live", "").body)["preview"].clone()), (0.3.into(), true.into()));
    let rejected = call("PUT", "/api/preview", "{}");
    assert_eq!(rejected.status, 400);
    assert!(shared.previewing());
    let cleared = call("DELETE", "/api/preview", "");
    assert_eq!((cleared.status, json(&cleared.body), shared.previewing()), (200, serde_json::json!({"preview": false}), false));
    assert_eq!(shared.generation(), 2);
}

#[test]
fn render_sets_security_headers() {
    for (status, reason) in [(200, "OK"), (400, "Bad Request"), (403, "Forbidden"), (404, "Not Found")] {
        let text = String::from_utf8(http::render(&http::Response { status, content_type: "text/plain", body: "hi".into() })).unwrap();
        assert!(text.starts_with(&format!("HTTP/1.1 {status} {reason}\r\n")));
        assert!(text.contains("Content-Security-Policy: default-src 'self'") && text.contains("Content-Length: 2") && text.ends_with("\r\n\r\nhi"));
    }
}

fn exchange(port: u16, raw: &[u8]) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(raw).unwrap();
    let mut text = String::new();
    stream.read_to_string(&mut text).unwrap();
    text
}

#[test]
fn live_server_serves_rejects_and_stops() {
    let shared = shared("server");
    assert!(Server::start(shared.clone(), "tok".into(), "256.0.0.1:0").is_err());
    let server = Server::start(shared, "tok".into(), "127.0.0.1:0").unwrap();
    assert_eq!(server.url(), format!("http://127.0.0.1:{}/#tok", server.port));
    let host = format!("Host: 127.0.0.1:{}", server.port);
    let page = exchange(server.port, format!("GET / HTTP/1.1\r\n{host}\r\n\r\n").as_bytes());
    assert!(page.starts_with("HTTP/1.1 200 OK") && page.contains("TCA-NEO bridge configuration"));
    let body = config::to_json(&config::defaults());
    let head = format!("PUT /api/config HTTP/1.1\r\n{host}\r\nX-Neo-Token: tok\r\nContent-Length: {}\r\n\r\n", body.len());
    // Split across two writes to exercise incremental reads.
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    stream.write_all(body.as_bytes()).unwrap();
    let mut text = String::new();
    stream.read_to_string(&mut text).unwrap();
    assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
    assert!(exchange(server.port, b"GARBAGE\r\n\r\n").starts_with("HTTP/1.1 400"));
    let oversized = format!("PUT /api/config HTTP/1.1\r\n{host}\r\nContent-Length: 70000\r\n\r\n{}", "x".repeat(70000));
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    let _ = stream.write_all(oversized.as_bytes());
    let mut text = String::new();
    let _ = stream.read_to_string(&mut text);
    assert!(text.is_empty() || text.starts_with("HTTP/1.1 400"));
    // Client that closes without sending anything; waiting for the reply guarantees the
    // zero-length read is served before the server shuts down.
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut text = String::new();
    stream.read_to_string(&mut text).unwrap();
    assert!(text.starts_with("HTTP/1.1 400"), "{text}");
    drop(server);
}
