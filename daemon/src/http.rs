//! Loopback-only configuration page. Every API call needs the per-launch token and an exact Host header.
use crate::{config, labels};
use crate::shared::Shared;
use serde_json::json;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub const INDEX: &str = include_str!("../web/index.html");
pub const SCRIPT: &str = include_str!("../web/app.js");
pub const STYLE: &str = include_str!("../web/style.css");
/// Static files: the page and its device diagrams.
pub const ASSETS: [(&str, &str, &str); 7] = [
    ("/", "text/html; charset=utf-8", INDEX),
    ("/app.js", "text/javascript; charset=utf-8", SCRIPT),
    ("/style.css", "text/css; charset=utf-8", STYLE),
    ("/device/neo.svg", "image/svg+xml", include_str!("../web/device/neo.svg")),
    ("/device/sidestick-right.svg", "image/svg+xml", include_str!("../web/device/sidestick-right.svg")),
    ("/device/sidestick-left.svg", "image/svg+xml", include_str!("../web/device/sidestick-left.svg")),
    ("/device/quadrant.svg", "image/svg+xml", include_str!("../web/device/quadrant.svg")),
];
const LIMIT: usize = 65536;

#[derive(Debug, Default, PartialEq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub host: String,
    pub token: String,
    pub body: String,
}

#[derive(Debug, PartialEq)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

fn reply(status: u16, content_type: &'static str, body: impl Into<String>) -> Response {
    Response { status, content_type, body: body.into() }
}

fn json_reply(status: u16, value: serde_json::Value) -> Response {
    reply(status, "application/json", value.to_string())
}

/// Parse one HTTP/1.1 request; None when malformed or oversized.
pub fn parse(raw: &[u8]) -> Option<Request> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&raw[..split]).ok()?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap().split(' ');
    let (method, target) = (first.next().unwrap().to_string(), first.next()?);
    let mut request = Request { method, path: target.split('?').next().unwrap().to_string(), ..Default::default() };
    let mut length = 0usize;
    for line in lines {
        let (name, value) = line.split_once(':')?;
        match name.trim().to_ascii_lowercase().as_str() {
            "host" => request.host = value.trim().to_string(),
            "x-neo-token" => request.token = value.trim().to_string(),
            "content-length" => length = value.trim().parse().ok()?,
            _ => {}
        }
    }
    let body = raw.get(split + 4..split + 4 + length)?;
    request.body = String::from_utf8(body.to_vec()).ok()?;
    Some(request)
}

/// Bytes still needed: headers incomplete → Some(0) keep reading; complete → total length.
pub fn expected_length(raw: &[u8]) -> Option<usize> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let length = head.lines().find_map(|l| l.strip_prefix("content-length:")).map_or(Some(0), |v| v.trim().parse().ok())?;
    Some(split + 4 + length)
}

pub fn handle(request: &Request, shared: &Shared, port: u16, token: &str) -> Response {
    if request.host != format!("127.0.0.1:{port}") {
        return reply(403, "text/plain", "Forbidden");
    }
    if let Some((_, content_type, body)) = ASSETS.iter().find(|(path, _, _)| request.method == "GET" && *path == request.path) {
        return reply(200, content_type, *body);
    }
    if !request.path.starts_with("/api/") {
        return reply(404, "text/plain", "Not found");
    }
    if request.token != token {
        return json_reply(403, json!({"error": "This page is out of date. Reopen Configure from the tray icon."}));
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/config") => {
            let (_, current) = shared.config();
            json_reply(200, json!({"config": current, "defaults": config::defaults(), "status": shared.status().label(), "links": *shared.links.lock().unwrap(), "profile": shared.config_path.display().to_string()}))
        }
        ("PUT", "/api/config") => match config::parse(&request.body).and_then(|c| shared.save(c)) {
            Ok(()) => json_reply(200, json!({"saved": true, "running": shared.status().running()})),
            Err(error) => json_reply(400, json!({"error": error})),
        },
        ("GET", "/api/labels") => match labels::load(&shared.labels_path()) {
            Ok(entries) => json_reply(200, json!({"labels": entries})),
            Err(error) => json_reply(400, json!({"error": error})),
        },
        ("PUT", "/api/labels") => match labels::parse(&request.body).and_then(|entries| labels::save(&shared.labels_path(), &entries).map(|()| entries.len())) {
            Ok(count) => json_reply(200, json!({"saved": count})),
            Err(error) => json_reply(400, json!({"error": error})),
        },
        ("GET", "/api/live") => json_reply(200, json!({"pressed": *shared.pressed.lock().unwrap(), "levers": *shared.levers.lock().unwrap(), "status": shared.status().label(), "links": *shared.links.lock().unwrap()})),
        _ => json_reply(404, json!({"error": "Unknown request."})),
    }
}

pub fn render(response: &Response) -> Vec<u8> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        _ => "Not Found",
    };
    format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'\r\nConnection: close\r\n\r\n{}",
        response.status,
        response.content_type,
        response.body.len(),
        response.body
    )
    .into_bytes()
}

fn serve(mut stream: TcpStream, shared: &Shared, port: u16, token: &str) {
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    let request = loop {
        match stream.read(&mut chunk) {
            Ok(count) if count > 0 && raw.len() + count <= LIMIT => raw.extend_from_slice(&chunk[..count]),
            _ => break None,
        }
        if expected_length(&raw).is_some_and(|total| raw.len() >= total) {
            break parse(&raw);
        }
    };
    let response = request.map_or_else(|| reply(400, "text/plain", "Bad request"), |r| handle(&r, shared, port, token));
    let _ = stream.write_all(&render(&response));
}

pub struct Server {
    pub port: u16,
    pub token: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(shared: Arc<Shared>, token: String, address: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_stop, thread_token) = (stop.clone(), token.clone());
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if thread_stop.load(Ordering::SeqCst) {
                    break;
                }
                serve(stream, &shared, port, &thread_token);
            }
        });
        Ok(Self { port, token, stop, thread: Some(thread) })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/#{}", self.port, self.token)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        self.thread.take().unwrap().join().unwrap();
    }
}
