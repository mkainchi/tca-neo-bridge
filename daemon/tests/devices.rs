mod common;
use common::*;
use neo_daemon::config;
use neo_daemon::device::{self, PipeServer, SoftwareDevice};
use neo_daemon::emulation::{Controller, Deps, Emulation, Notify, Source, RECONNECT_FAILED};
use neo_daemon::ffi;
use neo_daemon::hid::{self, Decoder, HidApi, InputState, Reader, Value};
use neo_daemon::hidhide::Hiding;
use neo_daemon::models;
use neo_daemon::neo;
use neo_daemon::protocol;
use neo_daemon::shared::{Links, Shared, Status};
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

/// Defaults plus the TCA profile stored explicitly, so its mapping can be changed.
fn with_tca() -> config::Config {
    let mut cfg = config::defaults();
    cfg.setups.insert(config::TCA_SETUP.into(), config::tca_profile());
    cfg
}

fn tca(cfg: &mut config::Config) -> &mut config::Profile {
    cfg.setups.get_mut(config::TCA_SETUP).unwrap()
}

fn ids(opened: &[hid::Opened]) -> Vec<String> {
    opened.iter().map(|o| o.info.id.clone()).collect()
}

fn close(opened: Vec<hid::Opened>) {
    for item in opened {
        item.close(&hid_api());
    }
}

#[test]
fn real_tables_construct() {
    let _g = guard();
    let _ = (HidApi::real(), device::DeviceApi::real());
    assert_eq!(device::multi_sz(&["ab", "c"]), [97, 98, 0, 99, 0, 0]);
    let mut field = [7u16; 4];
    ffi::fill(&mut field, "abcdef");
    assert_eq!(field, [97, 98, 99, 0]);
    assert_eq!(std::mem::size_of::<ffi::HIDP_BUTTON_CAPS>(), 72);
    assert_eq!(std::mem::size_of::<ffi::HIDP_VALUE_CAPS>(), 72);
}

#[test]
fn scan_keeps_joysticks_only_and_closes_the_rest() {
    let _g = guard();
    assert!(hid::find(&hid_api(), &[]).err().unwrap().contains("supported Thrustmaster HOTAS")); // nothing present
    state().list_invalid = true;
    assert!(hid::scan(&hid_api()).err().unwrap().contains("could not list"));
    assert!(hid::find(&hid_api(), &[]).err().unwrap().contains("could not list"));
    state().list_invalid = false;
    let mut bad_open = Interface::new(0x0405, 64);
    bad_open.open = false;
    let mut bad_attributes = Interface::new(0x0405, 64);
    bad_attributes.attributes = false;
    let mut bad_preparsed = Interface::new(0x0405, 64);
    bad_preparsed.preparsed = false;
    let mut bad_caps = Interface::new(0x0405, 64);
    bad_caps.caps = 0;
    let wrong_length = Interface::new(0x0405, 2);
    let mut vendor_collection = Interface::new(0x0407, 44);
    vendor_collection.usage_page = 0xff00;
    let mut keyboard = Interface::new(0x0407, 44);
    keyboard.usage = 6;
    let neo = Interface::generic(0x044f, models::NEO_PID, 8);
    let mut gamepad = Interface::generic(0x1234, 0x0001, 4);
    gamepad.usage = 5;
    let mut broken_generic = Interface::generic(0x044f, 0xb10a, 8);
    broken_generic.button_status = 0;
    {
        let mut s = state();
        s.interfaces = vec![bad_open, bad_attributes, bad_preparsed, bad_caps, wrong_length, vendor_collection, keyboard, neo, broken_generic, Interface::new(0x0406, 64), Interface::new(0x0407, 44), gamepad];
        for (index, interface) in s.interfaces.iter_mut().enumerate() {
            interface.path.push_str(&index.to_string());
        }
        s.detail_fail.insert(11);
    }
    let found = hid::scan(&hid_api()).unwrap();
    assert_eq!(ids(&found), ["tca-stick", "tca-quadrant"]); // the game pad's path could not be read
    assert!(state().list_destroyed >= 1);
    // Rejected handles are closed; preparsed data is freed when it was obtained.
    assert_eq!(state().closed.len(), 8);
    assert_eq!(state().freed_preparsed.len(), 6);
    let (stick, quadrant) = (&found[0], &found[1]);
    assert_eq!((stick.decoder.name(), quadrant.decoder.name()), ("tca-stick", "tca-quadrant"));
    assert_eq!((stick.info.name.as_str(), stick.info.pid, quadrant.length), ("TCA Sidestick Airbus", 0x0406, 44));
    close(found);
    state().detail_fail.clear();
    // An explicit list can take another vendor's game pad; auto mode leaves it alone.
    let found = hid::find(&hid_api(), &["1234-0001".into()]).unwrap();
    assert_eq!((ids(&found), found[0].info.name.as_str()), (vec!["1234-0001".to_string()], "Fake Stick"));
    close(found);
    assert_eq!(ids(&hid::find(&hid_api(), &[]).unwrap()), ["tca-stick", "tca-quadrant"]);
    assert_eq!(hid::list(&hid_api()).unwrap().len(), 3);
}

#[test]
fn identical_devices_are_numbered_and_names_fall_back() {
    let _g = guard();
    let mut second = Interface::generic(0x044f, 0x1234, 4);
    second.path.push('2');
    second.product = Some(format!("  {}\u{7}  ", "N".repeat(130))); // trimmed, control characters dropped, 64 kept
    let mut unnamed = Interface::generic(0x044f, 0x1234, 4);
    unnamed.path.push('3');
    unnamed.product = None;
    let mut blank = Interface::generic(0x044f, 0x1234, 4);
    blank.path.push('4');
    blank.product = Some("   ".into());
    state().interfaces = vec![Interface::generic(0x044f, 0x1234, 4), second, unnamed, blank];
    let devices = hid::list(&hid_api()).unwrap();
    let ids: Vec<_> = devices.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["044f-1234", "044f-1234#2", "044f-1234#3", "044f-1234#4"]);
    assert_eq!(devices[1].name, "N".repeat(64));
    assert_eq!((devices[2].name.as_str(), devices[3].name.as_str()), ("Joystick 044F:1234", "Joystick 044F:1234"));
    assert!(hid::selected(&[], &devices[0]));
    assert!(hid::selected(&["044f-1234#2".into()], &devices[1]));
    assert!(!hid::selected(&["044f-1234#2".into()], &devices[0]));
}

#[test]
fn generic_layouts_come_from_the_descriptor() {
    let _g = guard();
    let mut odd = Interface::generic(0x044f, 0xb10a, 0);
    odd.button_caps = vec![
        button_range(3, 1), // reversed range reads as its minimum
        ffi::HIDP_BUTTON_CAPS { UsagePage: 9, UsageMin: 200, ..Default::default() }, // capped at 128
        ffi::HIDP_BUTTON_CAPS { UsagePage: 0x0c, UsageMin: 250, ..Default::default() }, // consumer page: ignored
    ];
    odd.value_caps = vec![
        value(1, 0x30, 0, -1, 16),      // unsigned maximum returned sign-extended: 0..65535
        value(1, 0x31, -512, 511, 10),  // signed
        value(1, 0x32, 5, 5, 8),        // empty range: dropped
        value(1, 0x33, 0, 255, 0),      // no bits: dropped
        value(1, 0x34, 0, 255, 33),     // too wide: dropped
        value(2, 0xbb, 0, 1023, 10),    // Simulation throttle
        value(1, 0x30, 0, 7, 3),        // a second X is ignored
        value(0xff00, 0x01, 0, 7, 3),   // vendor value: ignored
        value(1, 0x39, 1, 4, 3),        // four-way hat
    ];
    state().interfaces = vec![odd];
    let found = hid::scan(&hid_api()).unwrap();
    let layout = &found[0].info.layout;
    assert_eq!(layout.buttons, 128);
    let axes: Vec<(&str, u32)> = layout.axes.iter().map(|a| (a.name.as_str(), a.max)).collect();
    assert_eq!(axes, [("x", 65535), ("y", 1023), ("throttle", 1023)]);
    assert!(layout.hat);
    close(found);
    // No button or value capabilities at all.
    let mut bare = Interface::generic(0x044f, 0xb10a, 0);
    bare.button_caps.clear();
    bare.value_caps.clear();
    state().interfaces = vec![bare];
    let found = hid::scan(&hid_api()).unwrap();
    assert_eq!(found[0].info.layout, models::Layout { buttons: 0, axes: vec![], hat: false });
    close(found);
    // A failing value capability read rejects the device.
    let mut broken = Interface::generic(0x044f, 0xb10a, 4);
    broken.value_status = 0;
    state().interfaces = vec![broken];
    assert!(hid::scan(&hid_api()).unwrap().is_empty());
}

#[test]
fn values_sign_extend_and_clamp() {
    let caps = |min, max, bits| ffi::HIDP_VALUE_CAPS { UsagePage: 1, LogicalMin: min, LogicalMax: max, BitSize: bits, ..Default::default() };
    let signed = Value::new(&caps(-128, 127, 8), 0x35).unwrap();
    assert_eq!((signed.relative(0x80), signed.relative(0x7f), signed.logical(0xff)), (0, 255, -1));
    let hat = Value::new(&caps(1, 8, 4), 0x39).unwrap();
    assert_eq!((hat.position(1), hat.position(8), hat.position(0), hat.position(15)), (Some(0), Some(7), None, None));
    assert_eq!(hat.relative(15), 7);
    let wide = Value::new(&caps(0, -1, 32), 0x30).unwrap();
    assert_eq!((wide.span, wide.relative(u32::MAX)), (u32::MAX, u32::MAX));
    assert_eq!(Value::new(&caps(0, 0, 8), 0x30), None);
    assert_eq!(Value::new(&caps(-5, -10, 8), 0x30), None); // signed and reversed: no range
}

#[test]
fn generic_reports_decode_buttons_axes_and_hat() {
    let _g = guard();
    let mut device = Interface::generic(0x044f, 0xb10a, 12);
    device.extra_usages = vec![0, 129]; // out-of-range usages are ignored
    state().interfaces = vec![device];
    let found = hid::scan(&hid_api()).unwrap();
    let api = hid_api();
    let decoder = &found[0].decoder;
    assert_eq!(decoder.name(), "generic");
    let decode = |buttons: &[u16], values: &[u32]| decoder.decode(&api, &generic_report(64, buttons, values));
    // X, Y, Z, Rz (signed 8-bit), hat.
    assert_eq!(decode(&[1, 12], &[0, 1023, 255, 0x80, 3]), Some(InputState { buttons: 1 | 1 << 11, axes: vec![0, 1023, 255, 0], hat: Some(3) }));
    assert_eq!(decode(&[], &[2000, 5, 0, 0x7f, 8]).unwrap(), InputState { buttons: 0, axes: vec![1023, 5, 0, 255], hat: None }); // clamped; hat null
    close(found);
    // Four-way hats report cardinals; other hat sizes are read as centred.
    for (max, raw, expected) in [(3, 1, Some(2)), (3, 4, None), (5, 2, None)] {
        let mut device = Interface::generic(0x044f, 0xb10a, 4);
        device.value_caps[3] = value(1, 0x39, 0, max, 4);
        state().interfaces = vec![device];
        let found = hid::scan(&hid_api()).unwrap();
        assert_eq!(found[0].decoder.decode(&api, &generic_report(64, &[], &[0, 0, 0, 0, raw])).unwrap().hat, expected, "{max} {raw}");
        close(found);
    }
    // HidP refusals (another report id, a failing field) skip the report.
    let mut refusing = Interface::generic(0x044f, 0xb10a, 4);
    refusing.usages_status = 0;
    let mut failing_axis = Interface::generic(0x044f, 0xb10a, 4);
    failing_axis.path.push('2');
    failing_axis.failing_usage = Some(0x31);
    let mut failing_hat = Interface::generic(0x044f, 0xb10a, 4);
    failing_hat.path.push('3');
    failing_hat.failing_usage = Some(0x39);
    let mut buttonless = Interface::generic(0x044f, 0xb10a, 4);
    buttonless.path.push('4');
    buttonless.button_caps.clear();
    buttonless.usages_status = 0; // never asked
    state().interfaces = vec![refusing, failing_axis, failing_hat, buttonless];
    let found = hid::scan(&hid_api()).unwrap();
    let report = generic_report(64, &[1], &[1, 2, 3, 4, 5]);
    let decoded: Vec<_> = found.iter().map(|o| o.decoder.decode(&api, &report).is_some()).collect();
    assert_eq!(decoded, [false, false, false, true]);
    assert_eq!(found[3].decoder.decode(&api, &report).unwrap().buttons, 0);
    close(found);
    assert_eq!(Decoder::TcaQuadrant.layout(), models::tca_quadrant().layout);
}

#[test]
fn reader_keeps_latest_decodable_report_and_flags_unexpected_failure() {
    let _g = guard();
    pair();
    let found = hid::scan(&hid_api()).unwrap();
    let mut found = found.into_iter();
    let (stick, quadrant) = (found.next().unwrap(), found.next().unwrap());
    let mut wrong_id = stick_report(0, 1);
    wrong_id[0] = 3;
    state().reads.insert(0, [Read::Data(stick_report(1, 1)), Read::Data(vec![1, 2]), Read::Data(wrong_id.clone()), Read::Fail].into());
    let reader = Reader::spawn(hid_api(), stick);
    wait_until(|| reader.failed.load(Ordering::SeqCst));
    assert_eq!(reader.snapshot(), hid::tca_stick(&stick_report(1, 1)));
    assert_eq!(*reader.raw.lock().unwrap(), Some(wrong_id)); // raw bytes keep the last full-length report
    drop(reader);
    let quiet = Reader::spawn(hid_api(), quadrant);
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(quiet.snapshot(), None);
    drop(quiet); // blocked read cancelled: not a failure
    assert_eq!(state().freed_preparsed.len(), 2);
}

#[test]
fn reader_stops_cleanly_while_reports_keep_arriving() {
    let _g = guard();
    pair();
    let stick = hid::scan(&hid_api()).unwrap().into_iter().next().unwrap();
    state().reads.insert(0, [Read::Forever(stick_report(4, 1))].into());
    let reader = Reader::spawn(hid_api(), stick);
    wait_until(|| reader.snapshot().is_some());
    drop(reader); // stop is observed between successful reads, not via a failed read
}

#[test]
fn probe_describes_devices_without_paths() {
    let _g = guard();
    assert!(hid::probe(hid_api(), Duration::from_millis(1)).unwrap().contains("No joysticks found"));
    let mut pad = Interface::generic(0x1234, 0x0001, 2);
    pad.path.push('2');
    pad.value_caps.clear();
    pad.fields.clear();
    let mut quiet = Interface::generic(0x044f, 0xb10a, 2);
    quiet.path.push('3');
    quiet.usages_status = 0;
    let mut silent = Interface::new(0x0407, 44);
    silent.path.push('4'); // never sends a report: no live sample, no raw bytes
    state().interfaces = vec![Interface::new(0x0405, 64), pad, quiet, silent];
    {
        let mut s = state();
        let mut stick = stick_report(1 | 1 << 16, 100);
        stick[7] = 2;
        s.reads.insert(0, [Read::Forever(stick)].into());
        s.reads.insert(1, [Read::Forever(generic_report(64, &[2], &[]))].into());
        s.reads.insert(2, [Read::Forever(vec![9; 64])].into()); // never decodes: raw only
    }
    let text = hid::probe(hid_api(), Duration::from_millis(30)).unwrap();
    assert!(!text.contains(r"\\?\") && !text.contains("hid#"), "{text}");
    assert!(text.contains("device tca-stick \"TCA Sidestick Airbus\" 044F:0405 decoder=tca-stick default=yes"), "{text}");
    assert!(text.contains("buttons 17, hat yes, axes: x 0..16383, y 0..16383, rz 0..255, slider 0..255"), "{text}");
    assert!(text.contains("live: buttons [1 17], hat right, x=100 y=8192 rz=128 slider=128"), "{text}");
    assert!(text.contains("device 1234-0001 \"Fake Stick\" 1234:0001 decoder=generic default=no (tick it under Axes and settings)"), "{text}");
    assert!(text.contains("buttons 2, hat no, axes: none"), "{text}");
    assert!(text.contains("live: buttons [2], hat centred, \n"), "{text}");
    assert!(text.contains("live: no decodable report yet"), "{text}");
    assert!(text.contains(&format!("raw: {}", "09".repeat(64))), "{text}");
    state().list_invalid = true;
    assert!(hid::probe(hid_api(), Duration::from_millis(1)).is_err());
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
        retry: Duration::from_millis(5),
        checks: Box::new(Vec::new),
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
fn validation_session_streams_without_hidhide_or_controllers() {
    let _g = guard();
    ready_device();
    let dir = temp("validation");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (deps, _) = deps(None, dir);
    let (notify, count) = counter();
    let session = Emulation::start(&deps, shared.clone(), Source::Validation, notify).unwrap();
    assert_eq!(*shared.links.lock().unwrap(), Some(Links { source: "validation", hidden: false, inputs: 0, virtual_neo: true }));
    wait_until(|| state().frames.len() > 3);
    drop(session);
    assert_eq!((shared.status(), count.load(Ordering::SeqCst)), (Status::Idle, 1));
    assert_eq!(*shared.links.lock().unwrap(), None);
    assert_eq!(*shared.devices.lock().unwrap(), None);
    assert_eq!(protocol::parse(state().frames.last().unwrap()), Some(neo::neutral()));
}

#[test]
fn tca_session_maps_hot_reloads_and_releases_on_disconnect() {
    let _g = guard();
    ready_device();
    pair();
    let dir = temp("tca");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (deps, fail) = deps(None, dir.clone());
    let (notify, notified) = counter();
    let session = Emulation::start(&deps, shared.clone(), Source::Hotas, notify).unwrap();
    assert!(dir.join("hidhide-session.json").exists());
    assert_eq!(*shared.links.lock().unwrap(), Some(Links { source: "hotas", hidden: true, inputs: 2, virtual_neo: true }));
    assert_eq!(shared.devices.lock().unwrap().as_ref().unwrap().len(), 2);
    std::thread::sleep(Duration::from_millis(10)); // no reports yet: neutral, nothing pressed
    assert!(shared.pressed.lock().unwrap().is_empty());
    {
        let mut s = state();
        s.reads.insert(0, [Read::Data(stick_report(1, 0))].into());
        s.reads.insert(1, [Read::Data(quadrant_report(65535))].into());
    }
    // Left lever at idle (raw 65535): safe start holds the throttle at centre until the lever reaches it.
    let pressed_report = |button: u16| neo::input_report([0, 32768, 32768, 32768, 32768, 32768, 32768], button, 8);
    wait_until(|| state().frames.iter().any(|f| protocol::parse(f) == Some(pressed_report(1))));
    assert_eq!(*shared.pressed.lock().unwrap(), ["tca-stick/b1"]);
    assert_eq!(*shared.levers.lock().unwrap(), Some([0.0, 1.0 - 32768.0 / 65535.0]));
    assert_eq!(shared.axes.lock().unwrap()["tca-quadrant"], [1.0, 2.0 * 32768.0 / 65535.0 - 1.0]);
    wait_until(|| shared.monitor.lock().unwrap().is_some_and(|m| m.roll == -1.0 && !m.armed));
    let mut changed = with_tca();
    tca(&mut changed).buttons.insert("tca-stick/b1".into(), vec![15]);
    shared.save(changed.clone()).unwrap();
    wait_until(|| state().frames.iter().any(|f| protocol::parse(f) == Some(pressed_report(1 << 14))));
    // A preview runs without being saved; its pause combo (s1 + s2) pauses input: neutral frames, a tray refresh.
    let mut preview = changed;
    tca(&mut preview).combos.pause = vec!["tca-stick/b1".into(), "tca-stick/b2".into()];
    shared.set_preview(Some(preview)).unwrap();
    let before = notified.load(Ordering::SeqCst);
    state().reads.insert(0, [Read::Data(stick_report(3, 0))].into());
    wait_until(|| shared.paused());
    assert!(notified.load(Ordering::SeqCst) > before);
    state().frames.clear();
    wait_until(|| state().frames.iter().any(|f| protocol::parse(f) == Some(neo::neutral())));
    assert!(state().frames.iter().all(|f| protocol::parse(f) == Some(neo::neutral())));
    state().reads.insert(0, [Read::Data(stick_report(1, 0))].into());
    // The next-profile combo (s4): without saved profiles it explains why; with one it switches to it.
    shared.set_paused(false);
    let mut cycling = with_tca();
    tca(&mut cycling).combos.next_profile = vec!["tca-stick/b4".into()];
    shared.set_preview(Some(cycling)).unwrap();
    state().reads.insert(0, [Read::Data(stick_report(8, 0))].into());
    wait_until(|| shared.notices.lock().unwrap().len() == 1);
    assert_eq!(shared.take_notice().map(|n| (n.title, n.error)), Some(("Profile not switched", true)));
    state().reads.insert(0, [Read::Data(stick_report(0, 0))].into());
    wait_until(|| shared.pressed.lock().unwrap().is_empty());
    let mut night = config::defaults();
    night.axes.roll.curve = 0.25;
    neo_daemon::profiles::save(&shared.profiles_dir(), "Night", &night).unwrap();
    state().reads.insert(0, [Read::Data(stick_report(8, 0))].into());
    wait_until(|| shared.notices.lock().unwrap().len() == 1);
    assert_eq!(shared.take_notice().map(|n| n.text), Some("Now flying with Night.".into()));
    assert_eq!((shared.config().1, shared.previewing()), (night, false));
    let mut hat_up = stick_report(1, 0);
    hat_up[7] = 0;
    state().reads.insert(0, [Read::Data(hat_up)].into());
    wait_until(|| *shared.pressed.lock().unwrap() == ["tca-stick/b1", "tca-stick/hat-up"]);
    // The quadrant is unplugged: controls are released, the virtual NEO stays, and the bridge waits for it.
    shared.notices.lock().unwrap().clear();
    {
        let mut s = state();
        s.interfaces.clear();
        s.reads.insert(1, [Read::Fail].into());
    }
    wait_until(|| shared.notices.lock().unwrap().len() == 1);
    assert_eq!(shared.take_notice().map(|n| (n.title, n.error)), Some(("Controller disconnected", true)));
    assert_eq!(*shared.links.lock().unwrap(), Some(Links { source: "hotas", hidden: true, inputs: 0, virtual_neo: true }));
    assert!(shared.pressed.lock().unwrap().is_empty() && shared.levers.lock().unwrap().is_none() && shared.monitor.lock().unwrap().is_none());
    std::thread::sleep(Duration::from_millis(30)); // several searches find nothing
    state().frames.clear();
    wait_until(|| state().frames.len() > 2);
    assert!(state().frames.iter().all(|f| protocol::parse(f) == Some(neo::neutral())));
    assert!(shared.links.lock().unwrap().is_some_and(|links| links.virtual_neo)); // the session is still running
    // It returns: hidden again, read again, and safe start holds the throttle until the lever reaches centre.
    pair();
    {
        let mut s = state();
        s.reads.insert(0, [Read::Data(stick_report(1, 0))].into());
        s.reads.insert(1, [Read::Data(quadrant_report(0))].into());
    }
    wait_until(|| shared.notices.lock().unwrap().len() == 1);
    assert_eq!(shared.take_notice().map(|n| (n.title, n.error)), Some(("Controller reconnected", false)));
    assert!(shared.links.lock().unwrap().is_some_and(|links| links.inputs == 2));
    wait_until(|| *shared.pressed.lock().unwrap() == ["tca-stick/b1"]);
    wait_until(|| shared.monitor.lock().unwrap().is_some_and(|m| !m.armed));
    // A device that comes back but cannot be hidden again stops the session.
    *fail.lock().unwrap() = Some("--dev-gaming".into());
    state().reads.insert(0, [Read::Fail].into());
    wait_until(|| shared.status() == Status::Error(RECONNECT_FAILED.into()));
    assert!(!dir.join("hidhide-session.json").exists());
    assert!(shared.pressed.lock().unwrap().is_empty());
    assert_eq!(*shared.levers.lock().unwrap(), None);
    // The session ends unpaused, without a preview or monitor.
    assert!(!shared.paused() && !shared.previewing() && shared.monitor.lock().unwrap().is_none());
    assert!(shared.axes.lock().unwrap().is_empty() && shared.devices.lock().unwrap().is_none());
    drop(session);
}

#[test]
fn generic_session_uses_the_convention_profile_and_the_device_list() {
    let _g = guard();
    ready_device();
    let mut other = Interface::generic(0x1234, 0x0001, 4);
    other.path.push('2');
    state().interfaces = vec![Interface::generic(0x044f, 0xb10a, 16), other];
    let dir = temp("generic");
    let mut cfg = config::defaults();
    cfg.safe_start = false;
    let shared = Arc::new(Shared::new(dir.join("config.json"), cfg.clone()));
    let (deps, _) = deps(None, dir);
    let (notify, _) = counter();
    // Auto selection: the T.16000M only (the other vendor's stick stays visible and unused).
    let session = Emulation::start(&deps, shared.clone(), Source::Hotas, notify.clone()).unwrap();
    assert_eq!(shared.links.lock().unwrap().unwrap().inputs, 1);
    // X full left, Y centre-ish, Z (the throttle) at 255, button 3, hat up: roll 0, throttle full, NEO 3, camera up.
    state().reads.insert(0, [Read::Forever(generic_report(64, &[3], &[0, 512, 255, 0, 0]))].into());
    let expected = |report: &neo::Report| report[4..6] == [0, 0] && report[8..10] == [0xff, 0xff] && report[12..14] == [0, 0] && report[14..16] == [0, 0x80] && (report[1] >> 4) & 0b100 != 0;
    wait_until(|| state().frames.iter().any(|f| protocol::parse(f).is_some_and(|r| expected(&r))));
    assert_eq!(*shared.pressed.lock().unwrap(), ["t16000m/b3", "t16000m/hat-up"]);
    assert_eq!(*shared.levers.lock().unwrap(), None);
    drop(session);
    // Explicit list: the other vendor's stick alone.
    cfg.devices = vec!["1234-0001".into()];
    shared.save(cfg).unwrap();
    let session = Emulation::start(&deps, shared.clone(), Source::Hotas, notify).unwrap();
    assert_eq!(shared.devices.lock().unwrap().as_ref().unwrap()[0].id, "1234-0001");
    drop(session);
}

#[test]
fn head_tracking_moves_the_camera_and_a_busy_port_is_reported() {
    let _g = guard();
    ready_device();
    pair();
    state().reads.insert(0, [Read::Forever(stick_report(0, 8192))].into());
    state().reads.insert(1, [Read::Forever(quadrant_report(32768))].into());
    let dir = temp("head");
    let mut cfg = config::defaults();
    cfg.head_tracking.enabled = true;
    cfg.head_tracking.port = 47_000 + (std::process::id() % 2000) as u16;
    let shared = Arc::new(Shared::new(dir.join("config.json"), cfg.clone()));
    let (first, _) = deps(None, dir.clone());
    let (notify, _) = counter();
    let session = Emulation::start(&first, shared.clone(), Source::Hotas, notify.clone()).unwrap();
    let sender = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut datagram = [0u8; 48];
    datagram[24..32].copy_from_slice(&22.5f64.to_le_bytes()); // half of the 45° yaw range
    wait_until(|| {
        sender.send_to(&datagram, ("127.0.0.1", cfg.head_tracking.port)).unwrap();
        shared.monitor.lock().unwrap().is_some_and(|m| m.head == Some([22.5, 0.0]))
    });
    let rx = shared.monitor.lock().unwrap().unwrap().axes[3];
    assert_eq!(rx, neo::unit16(neo::shape(0.5, &cfg.head_tracking.tuning)));
    // A second session on the same port cannot listen: it says so and still emulates.
    let blocked = Arc::new(Shared::new(temp("head2").join("config.json"), cfg.clone()));
    let (other, _) = deps(None, temp("head2"));
    drop(session);
    let holder = std::net::UdpSocket::bind(("127.0.0.1", cfg.head_tracking.port)).unwrap();
    let second = Emulation::start(&other, blocked.clone(), Source::Hotas, notify).unwrap();
    assert_eq!(blocked.take_notice().map(|n| n.title), Some("Head tracking off"));
    drop(second);
    drop(holder);
}

#[test]
fn sidestick_disconnect_reconnects_too() {
    let _g = guard();
    ready_device();
    pair();
    let dir = temp("stickfail");
    let shared = Arc::new(Shared::new(dir.join("config.json"), config::defaults()));
    let (deps, _) = deps(None, dir);
    let (notify, _) = counter();
    let session = Emulation::start(&deps, shared.clone(), Source::Hotas, notify).unwrap();
    // Only the sidestick is back at first: a different set, so the bridge keeps waiting for the whole pair.
    state().interfaces = vec![Interface::new(0x0405, 64)];
    state().reads.insert(0, [Read::Fail].into());
    wait_until(|| shared.notices.lock().unwrap().len() == 1);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(shared.notices.lock().unwrap().len(), 1);
    pair();
    wait_until(|| shared.notices.lock().unwrap().len() == 2);
    assert_eq!(shared.notices.lock().unwrap().iter().map(|n| n.title).collect::<Vec<_>>(), ["Controller disconnected", "Controller reconnected"]);
    assert!(shared.links.lock().unwrap().is_some_and(|links| links.virtual_neo)); // the session is still running
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
    assert_eq!(Emulation::start(&broken, shared.clone(), Source::Hotas, notify.clone()).err().unwrap(), "no hidhide");
    // HidHide refuses to start.
    let (failing, _) = deps(Some("--cloak-state"), dir.clone());
    assert!(Emulation::start(&failing, shared.clone(), Source::Hotas, notify.clone()).err().unwrap().contains("isolation"));
    // No controller present: HidHide restored.
    let (no_tca, _) = deps(None, dir.clone());
    assert!(Emulation::start(&no_tca, shared.clone(), Source::Hotas, notify.clone()).err().unwrap().contains("supported Thrustmaster HOTAS"));
    assert!(!dir.join("hidhide-session.json").exists());
    // Pipe unavailable.
    pair();
    state().pipe_invalid = true;
    assert!(Emulation::start(&no_tca, shared.clone(), Source::Hotas, notify.clone()).err().unwrap().contains("already in use"));
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
    assert_eq!(Emulation::start(&wrapped, shared.clone(), Source::Hotas, notify.clone()).err().unwrap(), neo_daemon::hidhide::ERROR);
    // Device failure with clean restore reports the device error.
    let (device_fails, _) = deps(None, temp("failures2"));
    assert!(Emulation::start(&device_fails, shared, Source::Hotas, notify).err().unwrap().contains("could not start"));
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
    let session = Emulation::start(&deps, shared.clone(), Source::Hotas, notify).unwrap();
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
    assert!(shared.warnings.lock().unwrap().is_empty() && shared.notices.lock().unwrap().is_empty());
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
