//! Runs the real executable so `main` is exercised (coverage collects child-process profiles).
use std::process::Command;

#[test]
fn binary_self_test_and_usage_exit_codes() {
    let exe = env!("CARGO_BIN_EXE_neo-daemon");
    assert_eq!(Command::new(exe).arg("--self-test").status().unwrap().code(), Some(0));
    assert_eq!(Command::new(exe).arg("--bogus").status().unwrap().code(), Some(2));
}
