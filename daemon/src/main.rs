#![windows_subsystem = "windows"]

/// Returning (rather than `process::exit`) lets the C runtime flush coverage profiles of the real binary.
fn main() -> std::process::ExitCode {
    let code = neo_daemon::app::run(&std::env::args().collect::<Vec<_>>(), neo_daemon::app::Apis::real());
    std::process::ExitCode::from(code as u8)
}
