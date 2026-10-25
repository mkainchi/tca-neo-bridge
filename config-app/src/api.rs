//! The daemon's loopback API: a minimal HTTP/1.1 client (the daemon answers each request and closes the connection).
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// The token is stale: this app was not started by the running daemon.
    Forbidden,
    /// The daemon refused the request and said why.
    Rejected(String),
    /// No answer, or one that is not HTTP.
    Unreachable,
}

/// One request: method, path and optional JSON body to the reply body. Tests substitute fakes.
pub trait Api: Send {
    fn call(&mut self, method: &str, path: &str, body: Option<&str>) -> Result<String, ApiError>;
}

pub struct Http {
    pub port: u16,
    pub token: String,
    pub timeout: Duration,
}

impl Http {
    pub fn new(port: u16, token: String) -> Self {
        Self { port, token, timeout: Duration::from_secs(2) }
    }

    fn exchange(&self, request: &[u8]) -> std::io::Result<Vec<u8>> {
        let mut stream = TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)), self.timeout)?;
        // Setting a timeout on a connected socket does not fail.
        let _ = stream.set_read_timeout(Some(self.timeout));
        let mut reply = Vec::new();
        stream.write_all(request).and_then(|()| stream.read_to_end(&mut reply)).map(|_| reply)
    }
}

/// The daemon checks the Host header against its own address and the token on every API call.
pub fn request(port: u16, token: &str, method: &str, path: &str, body: Option<&str>) -> String {
    let body = body.unwrap_or("");
    format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Neo-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Status and body of a complete reply; a refusal carries the daemon's message.
pub fn reply(raw: &[u8]) -> Result<String, ApiError> {
    let text = std::str::from_utf8(raw).map_err(|_| ApiError::Unreachable)?;
    let (head, body) = text.split_once("\r\n\r\n").ok_or(ApiError::Unreachable)?;
    let status = head.split(' ').nth(1).ok_or(ApiError::Unreachable)?;
    match status {
        "200" => Ok(body.to_string()),
        "403" => Err(ApiError::Forbidden),
        "400" | "404" => {
            let message = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| v["error"].as_str().map(str::to_string));
            Err(ApiError::Rejected(message.unwrap_or_else(|| "The daemon refused the request.".into())))
        }
        _ => Err(ApiError::Unreachable),
    }
}

impl Api for Http {
    fn call(&mut self, method: &str, path: &str, body: Option<&str>) -> Result<String, ApiError> {
        let raw = self.exchange(request(self.port, &self.token, method, path, body).as_bytes()).map_err(|_| ApiError::Unreachable)?;
        reply(&raw)
    }
}
