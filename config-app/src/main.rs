#![windows_subsystem = "windows"]

/// Returning (rather than `process::exit`) lets the C runtime flush coverage profiles of the real binary.
fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(neo_config::run(neo_config::Apis::real()) as u8)
}
