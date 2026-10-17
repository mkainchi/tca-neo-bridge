mod common;
use common::*;
use neo_daemon::driver::{self, DriverApi, DriverManager, Notice};
use neo_daemon::emulation::Notify;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-driver-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("driver")).unwrap();
    dir
}

fn manager(dir: &Path, listing: &Arc<Mutex<Option<String>>>, script: bool) -> (DriverManager, Arc<AtomicUsize>) {
    let script_path = dir.join("driver").join("install-driver.ps1");
    if script {
        std::fs::write(&script_path, "# test").unwrap();
    }
    let count = Arc::new(AtomicUsize::new(0));
    let inner = count.clone();
    let notify: Notify = Arc::new(move || {
        inner.fetch_add(1, Ordering::SeqCst);
    });
    (DriverManager::new(driver_api(), pnputil(listing.clone()), script_path, dir.join("driver-install.log"), notify), count)
}

#[test]
fn detection_matches_the_original_inf_name_in_any_language() {
    let _g = guard();
    let listing = Arc::new(Mutex::new(Some("Nom d’origine : NEOHID.INF".to_string())));
    assert_eq!(driver::detect(&pnputil(listing.clone())), Some(true));
    *listing.lock().unwrap() = Some("Original Name: other.inf".into());
    assert_eq!(driver::detect(&pnputil(listing.clone())), Some(false));
    *listing.lock().unwrap() = None;
    assert_eq!(driver::detect(&pnputil(listing)), None);
    let failing: neo_daemon::process::Runner = Box::new(|_, _| Err(std::io::ErrorKind::NotFound.into()));
    assert_eq!(driver::detect(&failing), None);
    let _ = DriverApi::real();
}

#[test]
fn install_runs_the_bundled_script_elevated_and_reports_success() {
    let _g = guard();
    let dir = temp("install");
    let listing = Arc::new(Mutex::new(Some("other.inf".to_string())));
    let (manager, count) = manager(&dir, &listing, true);
    manager.detect_async();
    manager.wait();
    assert_eq!(manager.menu(), (Some(false), false));
    // The script "installs" the package while it runs.
    *listing.lock().unwrap() = Some("neohid.inf".into());
    manager.install(77);
    manager.wait();
    let launched = driver_fake().launched.clone();
    assert_eq!(launched.len(), 1);
    let (verb, file, parameters) = &launched[0];
    assert_eq!((verb.as_str(), file.as_str()), ("runas", "powershell.exe"));
    assert!(parameters.contains("-ExecutionPolicy Bypass -File \"") && parameters.contains("install-driver.ps1\" -Log \"") && !parameters.contains("-Reinstall"));
    assert_eq!(driver_fake().closed, 1);
    assert_eq!(manager.menu(), (Some(true), false));
    assert_eq!(manager.take_notice(), Some(Notice { title: "Driver installed", text: "The virtual NEO driver is ready. Start emulation from the tray menu.".into(), error: false }));
    assert_eq!(manager.take_notice(), None);
    assert!(count.load(Ordering::SeqCst) >= 3); // detection, started, finished
}

#[test]
fn reinstall_passes_the_flag_reports_failure_and_ignores_repeat_clicks() {
    let _g = guard();
    let dir = temp("reinstall");
    let listing = Arc::new(Mutex::new(Some("neohid.inf".to_string())));
    let (manager, _) = manager(&dir, &listing, true);
    manager.detect_async();
    manager.wait();
    {
        let mut fake = driver_fake();
        fake.exit = 1;
        fake.hold = true;
    }
    manager.install(77);
    assert_eq!(manager.menu(), (Some(true), true));
    manager.install(77); // already installing: ignored
    assert_eq!(driver_fake().launched.len(), 1);
    driver_fake().hold = false;
    manager.wait();
    assert!(driver_fake().launched[0].2.ends_with(" -Reinstall"));
    let notice = manager.take_notice().unwrap();
    assert_eq!((notice.title, notice.error), ("Driver installation failed", true));
    assert!(notice.text.contains("driver-install.log"));
    assert_eq!(manager.menu(), (Some(true), false));
}

#[test]
fn missing_package_and_declined_uac_change_nothing() {
    let _g = guard();
    let dir = temp("missing");
    let listing = Arc::new(Mutex::new(None));
    let (manager, count) = manager(&dir, &listing, false);
    manager.install(77);
    let notice = manager.take_notice().unwrap();
    assert_eq!((notice.title, notice.error), ("Driver package missing", true));
    assert!(notice.text.contains("install-driver.ps1"));
    assert!(driver_fake().launched.is_empty());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let dir = temp("declined");
    let (manager, _) = manager_with_script(&dir, &listing);
    for uac in [Uac::Refuse, Uac::NoProcess] {
        driver_fake().uac = uac;
        manager.install(77);
        assert_eq!(manager.take_notice().unwrap().text, "The driver installation was cancelled.");
        assert_eq!(manager.menu(), (None, false));
    }
}

fn manager_with_script(dir: &Path, listing: &Arc<Mutex<Option<String>>>) -> (DriverManager, Arc<AtomicUsize>) {
    manager(dir, listing, true)
}
