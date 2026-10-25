//! Names the daemon and the configuration app agree on.

/// The configuration app, beside `neo-daemon.exe`.
pub const EXE: &str = "neo-config.exe";
/// Auto-reset: one Configure… brings the window forward once.
pub const SHOW_EVENT: &str = r"Local\tca-neo-bridge-config-show";
/// Manual-reset: set when the daemon quits.
pub const QUIT_EVENT: &str = r"Local\tca-neo-bridge-config-quit";
