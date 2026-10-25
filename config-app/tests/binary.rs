//! Runs the real executable the way the daemon does (coverage collects child-process profiles): the handshake on
//! standard input, a second copy refused, the show and quit events, and a daemon that stops answering.
mod common;
use common::{wait_until, TOKEN};
use neo_daemon::ffi::{self, HANDLE};
use neo_daemon::http;
use neo_daemon::shared::Shared;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[link(name = "kernel32")]
extern "system" {
    fn ResetEvent(event: HANDLE) -> ffi::BOOL;
}

/// The daemon's request handling on a listener of our own, recording every path asked for.
struct Daemon {
    port: u16,
    paths: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Daemon {
    fn start() -> Self {
        let dir = std::env::temp_dir().join(format!("neo-config-binary-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let shared = Shared::new(dir.join("config.json"), neo_daemon::config::defaults());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (paths, stop) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicBool::new(false)));
        let (seen, stopping) = (paths.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                let mut raw = Vec::new();
                let mut chunk = [0u8; 4096];
                while !http::expected_length(&raw).is_some_and(|total| raw.len() >= total) {
                    let count = stream.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    raw.extend_from_slice(&chunk[..count]);
                }
                let request = http::parse(&raw).unwrap();
                seen.lock().unwrap().push(request.path.clone());
                let _ = stream.write_all(&http::render(&http::handle(&request, &shared, port, TOKEN)));
            }
        });
        Self { port, paths, stop, thread: Some(thread) }
    }

    fn asked(&self, path: &str) -> usize {
        self.paths.lock().unwrap().iter().filter(|p| *p == path).count()
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}

fn launch(port: u16) -> Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_neo-config")).stdin(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    writeln!(input, "{port} {TOKEN}").unwrap();
    child
}

fn exit_code(child: &mut Child) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status.code().unwrap();
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("neo-config did not exit");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn event(name: &str, manual: bool) -> HANDLE {
    let name = ffi::wide(name);
    let event = unsafe { ffi::CreateEventW(std::ptr::null(), manual.into(), 0, name.as_ptr()) };
    assert_ne!(event, 0);
    event
}

#[test]
fn the_real_app_follows_the_daemons_lifecycle() {
    let mut daemon = Daemon::start();
    let (show, quit) = (event(neo_config::launch::SHOW_EVENT, false), event(neo_config::launch::QUIT_EVENT, true));
    // Another copy holds the mutex: this one exits at once.
    let name = ffi::wide(neo_config::launch::MUTEX);
    let mutex = unsafe { ffi::CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
    assert_eq!(exit_code(&mut launch(daemon.port)), 3);
    unsafe { ffi::CloseHandle(mutex) };
    // Opened: it loads the profile and polls live input; Configure… again brings it forward; Quit closes it.
    let mut app = launch(daemon.port);
    wait_until(|| daemon.asked("/api/config") == 1 && daemon.asked("/api/live") >= 2);
    unsafe { ffi::SetEvent(show) };
    std::thread::sleep(Duration::from_millis(200));
    assert!(app.try_wait().unwrap().is_none());
    unsafe { ffi::SetEvent(quit) };
    assert_eq!(exit_code(&mut app), 0);
    // The daemon stops answering: the app closes itself.
    unsafe { ResetEvent(quit) };
    let mut app = launch(daemon.port);
    wait_until(|| daemon.asked("/api/config") == 2);
    daemon.stop();
    assert_eq!(exit_code(&mut app), 0);
    unsafe {
        ffi::CloseHandle(show);
        ffi::CloseHandle(quit);
    }
}
