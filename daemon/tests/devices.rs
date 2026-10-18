mod common;
use common::*;
use neo_daemon::config;
use neo_daemon::device::{self, PipeServer, SoftwareDevice};
use neo_daemon::emulation::{Controller, Deps, Emulation, Notify, Source, DISCONNECTED};
use neo_daemon::ffi;
use neo_daemon::hidhide::Hiding;
use neo_daemon::neo;
use neo_daemon::protocol;
use neo_daemon::shared::{Links, Shared, Status};
use neo_daemon::tca::{self, HidApi, Reader, Role};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn pair() {
    let mut s = state();
    s.interfaces = vec![Interface::new(0x0405, 64), Interface::new(0x0407, 44)];
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-devices-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn real_tables_construct() {
    let _g = guard();
    let _ = (HidApi::real(), device::DeviceApi::real());
    assert_eq!(device::multi_sz(&["ab", "c"]), [97, 98, 0, 99, 0, 0]);
    let mut field = [7u16; 4];
    ffi::fill(&mut field, "abcdef");
    assert_eq!(field, [97, 98, 99, 0]);
}

#[test]
fn roles_use_generic_attributes_and_lengths_only() {
    let attributes = |vid, pid| ffi::HIDD_ATTRIBUTES { VendorID: vid, ProductID: pid, ..Default::default() };
    assert_eq!(tca::role(&attributes(0x044f, 0x0405), 64), Some(Role::Stick));
    assert_eq!(tca::role(&attributes(0x044f, 0x0406), 64), Some(Role::Stick));
    assert_eq!(tca::role(&attributes(0x044f, 0x0407), 44), Some(Role::Quadrant));
    assert_eq!(tca::role(&attributes(0x044f, 0x0407), 64), None);
    assert_eq!(tca::role(&attributes(0x1234, 0x0405), 64), None);
}

#[test]
fn find_pair_opens_exactly_one_of_each_and_closes_the_rest() {
    let _g = guard();
    assert!(tca::find_pair(&hid_api()).is_err()); // nothing present
    state().interfaces = vec![Interface::new(0x0405, 64)];
    assert!(tca::find_pair(&hid_api()).is_err()); // stick without quadrant
    state().interfaces.clear();
    state().list_invalid = true;
    assert!(tca::find_pair(&hid_api()).err().unwrap().contains("could not list"));
    state().list_invalid = false;
    let mut bad_open = Interface::new(0x0405, 64);
    bad_open.open = false;
    let mut bad_attributes = Interface::new(0x0405, 64);
    bad_attributes.attributes = false;
    let mut bad_preparsed = Interface::new(0x0405, 64);
    bad_preparsed.preparsed = false;
    let mut bad_caps = Interface::new(0x0405, 64);
    bad_caps.caps = 0;
    let mut other_collection = Interface::new(0x0405, 2);
    other_collection.path.push('2');
    let mut skipped = Interface::new(0x0406, 64);
    skipped.path.push('3');
    {
        let mut s = state();
        s.interfaces = vec![bad_open, bad_attributes, bad_preparsed, bad_caps, other_collection, skipped, Interface::new(0x0407, 44)];
        s.detail_fail.insert(5);
    }
    // Paths are identical for the first four, so they resolve to the first (unopenable) entry; use distinct paths.
    for (index, interface) in state().interfaces.iter_mut().enumerate() {
        interface.path.push_str(&index.to_string());
    }
    // Only the quadrant qualifies: no stick → error, and the quadrant handle is closed.
    let error = tca::find_pair(&hid_api()).err().unwrap();
    assert!(error.contains("exactly one"));
    assert!(state().list_destroyed >= 1);
    {
        let mut s = state();
        s.detail_fail.clear();
        s.closed.clear();
    }
    let (stick, quadrant) = tca::find_pair(&hid_api()).unwrap();
    assert_eq!((stick.role, quadrant.role), (Role::Stick, Role::Quadrant));
    assert_eq!(state().closed.len(), 4); // attributes, preparsed, caps, wrong-length collections
}

#[test]
fn reader_keeps_latest_valid_report_and_flags_unexpected_failure() {
    let _g = guard();
    pair();
    let (stick, quadrant) = tca::find_pair(&hid_api()).unwrap();
    {
        let mut s = state();
        let mut wrong_id = stick_report(0, 1);
        wrong_id[0] = 3;
        s.reads.insert(0, [Read::Data(stick_report(1, 1)), Read::Data(vec![1, 2]), Read::Data(wrong_id), Read::Fail].into());
    }
    let reader = Reader::spawn(hid_api(), stick);
    wait_until(|| reader.failed.load(Ordering::SeqCst));
    assert_eq!(reader.snapshot(), Some(stick_report(1, 1)));
    drop(reader);
    let quiet = Reader::spawn(hid_api(), quadrant);
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(quiet.snapshot(), None);
    drop(quiet); // blocked read cancelled: not a failure
}

#[test]
fn reader_stops_cleanly_while_reports_keep_arriving() {
    let _g = guard();
    pair();
    let (stick, _quadrant) = tca::find_pair(&hid_api()).unwrap();
    state().reads.insert(0, [Read::Forever(stick_report(4, 1))].into());
    let reader = Reader::spawn(hid_api(), stick);
    wait_until(|| reader.snapshot().is_some());
    drop(reader); // stop is observed between successful reads, not via a failed read
}

#[test]
fn software_device_outcomes() {
    let _g = guard();
    let api = device_api();
    state().sw_result = Some(0);
    let created = SoftwareDevice::create(api, Duration::from_secs(1)).unwrap();
    assert_eq!(state().sw_hardware, device::HARDWARE_ID);
    drop(created);
    assert_eq!(state().sw_closed, 1);
    state().sw_result = Some(-1);
    assert!(SoftwareDevice::create(api, Duration::from_secs(1)).err().unwrap().contains("could not start"));
    state().sw_result = None;
    assert!(SoftwareDevice::create(api, Duration::from_millis(10)).err().unwrap().contains("did not appear"));
    state().sw_hr = 0x8007_0005_u32 as i32;
    assert!(SoftwareDevice::create(api, Duration::from_secs(1)).err().unwrap().contains("administrator"));
    state().sw_hr = -2;
    assert!(SoftwareDevice::create(api, Duration::from_secs(1)).err().unwrap().contains("0xfffffffe"));
    assert_eq!(state().sw_closed, 3); // failed creations after a handle was returned are closed
}

#[test]
fn pipe_server_streams_frames_reconnects_and_ends_neutral() {
    let _g = guard();
    let api = device_api();
    state().sddl_fail = true;
    assert!(PipeServer::start(api, "p", Duration::from_millis(1)).err().unwrap().contains("security descriptor"));
    {
        let mut s = state();
        s.sddl_fail = false;
        s.pipe_invalid = true;
    }
    assert!(PipeServer::start(api, "p", Duration::from_millis(1)).err().unwrap().contains("already in use"));
    {
        let mut s = state();
        s.pipe_invalid = false;
        // refused, then already-connected client, write failure, then a normal connection.
        s.connect = [(0, 2), (0, ffi::ERROR_PIPE_CONNECTED), (1, 0)].into();
        s.write_fail_after = Some(2);
    }
    let server = PipeServer::start(api, "p", Duration::from_millis(1)).unwrap();
    let report = neo::input_report([1; 7], 2, 3);
    server.send(report);
    wait_until(|| state().frames.len() > 6 && server.connected.load(Ordering::SeqCst));
    drop(server);
    let s = state();
    assert!(s.frames.iter().any(|f| protocol::parse(f) == Some(report)));
    assert!(s.disconnects >= 2);
    assert_eq!(s.freed, 2);
}

fn deps(runner_fail: Option<&str>, hidden_dir: PathBuf) -> (Deps, Arc<Mutex<Option<String>>>) {
    let fail = Arc::new(Mutex::new(runner_fail.map(String::from)));
    let runner_fail = fail.clone();
    let deps = Deps {
        hid: hid_api(),
        device: device_api(),
        pipe_name: "p".into(),
        hiding: Box::new(move || Hiding::new(Some("HidHideCLI.exe".into()), hidhide_runner(runner_fail.clone()), &hidden_dir, &["neo-daemon.exe".into()])),
        period: Duration::from_millis(1),
        device_timeout: Duration::from_millis(50),
    };
    (deps, fail)
}

fn counter() -> (Notify, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let inner = count.clone();
    (Arc::new(move || {
        inner.fetch_add(1, Ordering::SeqCst);
    }), count)
}

fn ready_device() {
    let mut s = state();
    s.sw_result = Some(0);
    s.connect = [(1, 0)].into();
}

#[test]
fn validation_session_streams_without_hidhide_or_tca() {
    let _g = guard();
    ready_device();
    let dir = temp("validation");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (deps, _) = deps(None, dir);
    let (notify, count) = counter();
    let session = Emulation::start(&deps, shared.clone(), Source::Validation, notify).unwrap();
    assert_eq!(*shared.links.lock().unwrap(), Some(Links { source: "validation", hidden: false, tca: false, virtual_neo: true }));
    wait_until(|| state().frames.len() > 3);
    drop(session);
    assert_eq!((shared.status(), count.load(Ordering::SeqCst)), (Status::Idle, 1));
    assert_eq!(*shared.links.lock().unwrap(), None);
    assert_eq!(protocol::parse(state().frames.last().unwrap()), Some(neo::neutral()));
}

#[test]
fn tca_session_maps_hot_reloads_and_releases_on_disconnect() {
    let _g = guard();
    ready_device();
    pair();
    let dir = temp("tca");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (deps, _) = deps(None, dir.clone());
    let (notify, _) = counter();
    let session = Emulation::start(&deps, shared.clone(), Source::Tca, notify).unwrap();
    assert!(dir.join("hidhide-session.json").exists());
    assert_eq!(*shared.links.lock().unwrap(), Some(Links { source: "tca", hidden: true, tca: true, virtual_neo: true }));
    std::thread::sleep(Duration::from_millis(10)); // no reports yet: neutral, nothing pressed
    assert!(shared.pressed.lock().unwrap().is_empty());
    {
        let mut s = state();
        s.reads.insert(0, [Read::Data(stick_report(1, 0))].into());
        s.reads.insert(1, [Read::Data(quadrant_report(65535))].into());
    }
    // Left lever at idle (raw 65535, the NEO Z idle value too); default throttle is not inverted.
    let pressed_report = |button: u16| neo::input_report([0, 32768, 65535, 32768, 32768, 32768, 32768], button, 8);
    wait_until(|| state().frames.iter().any(|f| protocol::parse(f) == Some(pressed_report(1))));
    assert_eq!(*shared.pressed.lock().unwrap(), ["s1"]);
    assert_eq!(*shared.levers.lock().unwrap(), Some([0.0, 1.0 - 32768.0 / 65535.0]));
    let mut changed = config::defaults();
    changed.buttons.insert("s1".into(), vec![15]);
    shared.save(changed).unwrap();
    wait_until(|| state().frames.iter().any(|f| protocol::parse(f) == Some(pressed_report(1 << 14))));
    let mut hat_up = stick_report(1, 0);
    hat_up[7] = 0;
    state().reads.insert(0, [Read::Data(hat_up)].into());
    wait_until(|| *shared.pressed.lock().unwrap() == ["s1", "hat-up"]);
    state().reads.insert(1, [Read::Fail].into());
    wait_until(|| shared.status() == Status::Error(DISCONNECTED.into()));
    assert!(!dir.join("hidhide-session.json").exists());
    assert!(shared.pressed.lock().unwrap().is_empty());
    assert_eq!(*shared.levers.lock().unwrap(), None);
    drop(session);
}

#[test]
fn sidestick_disconnect_also_stops_the_session() {
    let _g = guard();
    ready_device();
    pair();
    let dir = temp("stickfail");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (deps, _) = deps(None, dir);
    let (notify, _) = counter();
    let session = Emulation::start(&deps, shared.clone(), Source::Tca, notify).unwrap();
    state().reads.insert(0, [Read::Fail].into());
    wait_until(|| shared.status() == Status::Error(DISCONNECTED.into()));
    drop(session);
}

#[test]
fn startup_failures_release_everything_and_report_the_first_cause() {
    let _g = guard();
    let dir = temp("failures");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (notify, _) = counter();
    // Hiding factory error.
    let mut broken = deps(None, dir.clone()).0;
    broken.hiding = Box::new(|| Err("no hidhide".into()));
    assert_eq!(Emulation::start(&broken, shared.clone(), Source::Tca, notify.clone()).err().unwrap(), "no hidhide");
    // HidHide refuses to start.
    let (failing, _) = deps(Some("--cloak-state"), dir.clone());
    assert!(Emulation::start(&failing, shared.clone(), Source::Tca, notify.clone()).err().unwrap().contains("isolation"));
    // No TCA present: HidHide restored.
    let (no_tca, _) = deps(None, dir.clone());
    assert!(Emulation::start(&no_tca, shared.clone(), Source::Tca, notify.clone()).err().unwrap().contains("exactly one"));
    assert!(!dir.join("hidhide-session.json").exists());
    // Pipe unavailable.
    pair();
    state().pipe_invalid = true;
    assert!(Emulation::start(&no_tca, shared.clone(), Source::Tca, notify.clone()).err().unwrap().contains("already in use"));
    state().pipe_invalid = false;
    // Device missing, and restoring HidHide then fails: the cleanup failure wins.
    let (restore_fails, fail) = deps(None, dir.clone());
    state().sw_result = Some(-1);
    *fail.lock().unwrap() = None;
    let fail_later = fail.clone();
    let mut wrapped = restore_fails;
    let original = std::mem::replace(&mut wrapped.hiding, Box::new(|| Err(String::new())));
    wrapped.hiding = Box::new(move || {
        let hiding = original();
        *fail_later.lock().unwrap() = Some("--dev-unhide".into());
        hiding
    });
    assert_eq!(Emulation::start(&wrapped, shared.clone(), Source::Tca, notify.clone()).err().unwrap(), neo_daemon::hidhide::ERROR);
    // Device failure with clean restore reports the device error.
    let (device_fails, _) = deps(None, temp("failures2"));
    assert!(Emulation::start(&device_fails, shared, Source::Tca, notify).err().unwrap().contains("could not start"));
}

#[test]
fn restore_failure_after_a_clean_stop_is_reported() {
    let _g = guard();
    ready_device();
    pair();
    let dir = temp("stopfail");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (deps, fail) = deps(None, dir);
    let (notify, _) = counter();
    let session = Emulation::start(&deps, shared.clone(), Source::Tca, notify).unwrap();
    *fail.lock().unwrap() = Some("--dev-unhide".into());
    drop(session);
    assert_eq!(shared.status(), Status::Error(neo_daemon::hidhide::ERROR.into()));
}

#[test]
fn controller_start_stop_errors_and_shutdown() {
    let _g = guard();
    let dir = temp("controller");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (deps, _) = deps(None, dir);
    let (notify, count) = counter();
    let controller = Controller::new(shared.clone(), deps, Source::Validation, notify);
    controller.stop(); // not emulating: ignored
    assert_eq!(count.load(Ordering::SeqCst), 0);
    state().sw_result = Some(-1);
    controller.start();
    controller.wait();
    assert!(matches!(shared.status(), Status::Error(_)));
    ready_device();
    controller.start();
    controller.start(); // ignored while starting/emulating
    controller.wait();
    assert_eq!(shared.status(), Status::Emulating);
    controller.stop();
    controller.wait();
    assert_eq!(shared.status(), Status::Idle);
    state().connect = [(1, 0)].into();
    controller.start();
    controller.wait();
    controller.shutdown();
    assert_eq!(shared.status(), Status::Idle);
    assert!(count.load(Ordering::SeqCst) >= 8);
}
