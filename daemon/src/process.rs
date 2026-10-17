//! Bounded child processes (HidHide CLI, pnputil) without console windows.
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Exit code (None if killed) and trimmed stdout.
pub type Runner = Box<dyn Fn(&Path, &[String]) -> io::Result<(Option<i32>, String)> + Send + Sync>;

pub fn run_cli(cli: &Path, args: &[String], timeout: Duration) -> io::Result<(Option<i32>, String)> {
    use std::os::windows::process::CommandExt;
    let mut child = Command::new(cli).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(0x0800_0000).spawn()?;
    let deadline = Instant::now() + timeout;
    let exited = loop {
        match child.try_wait() {
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) | Err(_) => break false,
            Ok(Some(_)) => break true,
        }
    };
    if !exited {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::ErrorKind::TimedOut.into());
    }
    child.wait_with_output().map(|output| (output.status.code(), String::from_utf8_lossy(&output.stdout).trim().to_string()))
}

pub fn runner(timeout: Duration) -> Runner {
    Box::new(move |cli, args| run_cli(cli, args, timeout))
}
