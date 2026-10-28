//! Names the daemon and the configuration app agree on. The experimental line uses its own, so its window never
//! answers a stable TCA-NEO bridge running on the same PC.

/// The configuration app, beside `neo-daemon.exe`.
pub const EXE: &str = "neo-config.exe";
/// Auto-reset: one Configure… brings the window forward once.
pub const SHOW_EVENT: &str = r"Local\any-tm-hotas-bridge-config-show";
/// Manual-reset: set when the daemon quits.
pub const QUIT_EVENT: &str = r"Local\any-tm-hotas-bridge-config-quit";
