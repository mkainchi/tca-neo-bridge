//! The settings draft: fields, validation, the quadrant levers' users, captures, Try in game and saving.
use neo_common::config::{Config, YawSource};
use neo_config::settings::{Axis, Capture, Field, Kind, LeverUser, Settings, Tone};
use neo_config::worker::Request;

fn defaults() -> Config {
    neo_daemon::config::defaults()
}

fn loaded() -> Settings {
    let mut s = Settings::default();
    s.show(defaults());
    s
}

fn message(s: &Settings) -> (&str, Tone) {
    let (text, tone) = s.message.as_ref().expect("a message");
    (text.as_str(), *tone)
}

#[test]
fn every_field_reads_and_writes_its_setting() {
    for axis in Axis::ALL {
        let mut config = defaults();
        for (index, field) in Field::ALL.into_iter().enumerate() {
            let (low, high, step, factor, _) = field.spec();
            assert!(low < high && step > 0.0 && factor > 0.0);
            let value = (low + step * (1.0 + index as f64 % 3.0)).min(high) / factor;
            field.set(&mut config, axis, value);
            assert!((field.get(&config, axis) - value).abs() < 1e-9, "{field:?}");
        }
    }
    assert_eq!(Axis::ALL.map(Axis::name), ["Roll", "Pitch", "Yaw"]);
    assert_eq!((Axis::Roll.copy_targets(), Axis::Pitch.copy_targets(), Axis::Yaw.copy_targets()), (&[Axis::Pitch][..], &[Axis::Roll][..], &[Axis::Roll, Axis::Pitch][..]));
    assert_eq!([Kind::Precision, Kind::Shift, Kind::Pause, Kind::Next].map(Kind::text), ["precision aiming", "the shift layer", "pause", "next-profile"]);
}

#[test]
fn fields_show_the_draft_and_take_typed_numbers() {
    let mut empty = Settings::default();
    empty.fill();
    assert!(empty.fields.is_empty());
    assert!(empty.edit("Unsaved changes.", |_| unreachable!()).is_empty());
    let mut s = loaded();
    assert_eq!((s.fields[&Field::Deadzone].as_str(), s.fields[&Field::HeadPort].as_str()), ("4", "4242"));
    s.type_number(Field::Deadzone, "12");
    assert_eq!((s.draft.as_ref().unwrap().axes.roll.deadzone, s.dirty, message(&s)), (0.12, true, ("Unsaved changes.", Tone::Dirty)));
    // Half-typed text stays in the field and leaves the draft alone.
    s.type_number(Field::Deadzone, "1x");
    assert_eq!((s.fields[&Field::Deadzone].as_str(), s.draft.as_ref().unwrap().axes.roll.deadzone), ("1x", 0.12));
    s.type_number(Field::Deadzone, "inf");
    assert_eq!(s.draft.as_ref().unwrap().axes.roll.deadzone, 0.12);
    s.set_axis(Axis::Yaw);
    assert_eq!(s.fields[&Field::Deadzone], "4");
    s.type_number(Field::HeadPort, "5000");
    s.type_number(Field::CameraRamp, "250");
    assert_eq!((s.draft.as_ref().unwrap().head_tracking.port, s.draft.as_ref().unwrap().camera.ramp_ms), (5000, 250));
}

#[test]
fn validation_flags_ranges_and_the_rules_between_fields() {
    let mut s = loaded();
    s.fields.insert(Field::Deadzone, "60".into());
    s.fields.insert(Field::LongPress, "".into());
    assert!(!s.validate());
    assert_eq!(s.errors[&Field::Deadzone], "Enter a number from 0 to 50.");
    assert_eq!(s.error(Field::DoubleTap).map(String::as_str), Some("Enter a number from 200 to 2000."));
    assert_eq!(s.error(Field::Curve), None);
    assert_eq!(message(&s), ("Fix the highlighted fields, then save.", Tone::Error));
    let rules = [
        (&[(Field::Deadzone, "50"), (Field::Saturation, "50")][..], Field::Saturation, "Must be beyond the deadzone."),
        (&[(Field::Repeat, "80")][..], Field::Repeat, "Repeat every must be longer than the pulse length."),
        (&[(Field::Idle, "48")][..], Field::Cruise, "Needs idle < cruise \u{2212} band and cruise + band < boost."),
        (&[(Field::Boost, "52")][..], Field::Cruise, "Needs idle < cruise \u{2212} band and cruise + band < boost."),
        (&[(Field::SplitBack, "75")][..], Field::SplitBack, "Must be below the forward lever threshold."),
    ];
    for (values, field, text) in rules {
        let mut s = loaded();
        for (field, value) in values {
            s.fields.insert(*field, value.to_string());
        }
        assert!(!s.validate());
        assert_eq!(s.errors.get(&field).map(String::as_str), Some(text));
    }
    // Valid: the draft takes exactly what the fields show.
    let mut s = loaded();
    s.fields.insert(Field::Curve, "30".into());
    assert!(s.validate());
    assert_eq!(s.draft.as_ref().unwrap().axes.roll.curve, 0.3);
    let mut empty = Settings { fields: s.fields.clone(), ..Settings::default() };
    assert!(empty.validate());
}

#[test]
fn the_quadrant_levers_have_one_user_at_a_time() {
    let mut s = loaded();
    s.set_split(false);
    assert_eq!(message(&s).0, "Unsaved changes.");
    s.set_camera_lever(true);
    assert_eq!(message(&s).0, "Unsaved changes.");
    let draft = |s: &Settings| s.draft.clone().unwrap();
    s.set_yaw_source(YawSource::LeverDifferential);
    assert_eq!((draft(&s).camera.lever, message(&s).0), (false, "Unsaved changes: the camera lever turned off, since it uses the quadrant levers too."));
    s.set_camera_lever(false);
    s.set_split(true);
    assert_eq!((draft(&s).yaw_source, message(&s).0), (YawSource::Twist, "Unsaved changes: lever yaw turned off, since it uses the quadrant levers too."));
    s.set_yaw_source(YawSource::Off);
    let draft = s.draft.as_mut().unwrap();
    draft.split_high_g.enabled = true;
    draft.yaw_source = YawSource::LeverDifferential;
    s.set_camera_lever(true);
    assert_eq!(message(&s).0, "Unsaved changes: split-lever High-G and lever yaw turned off, since it uses the quadrant levers too.");
    let mut empty = Settings::default();
    assert!(empty.exclusive(LeverUser::Split).is_empty());
}

#[test]
fn axes_copy_and_reset_and_everything_resets() {
    let mut s = loaded();
    s.type_number(Field::Curve, "40");
    s.copy_axis();
    assert_eq!((s.draft.as_ref().unwrap().axes.pitch.curve, message(&s).0), (0.4, "Unsaved: Roll settings copied to pitch."));
    s.set_axis(Axis::Yaw);
    s.copy_axis();
    assert_eq!(message(&s).0, "Unsaved: Yaw settings copied to roll and pitch.");
    assert_eq!(s.draft.as_ref().unwrap().axes.roll.curve, 0.0);
    s.set_axis(Axis::Pitch);
    s.type_number(Field::Curve, "20");
    s.reset_axis(&defaults());
    assert_eq!((s.fields[&Field::Curve].as_str(), message(&s).0), ("0", "Unsaved: Pitch back to its defaults."));
    let mut changed = defaults();
    changed.buttons.clear();
    s.show(changed);
    s.reset(&defaults());
    assert_eq!((s.draft.clone(), message(&s).0), (Some(defaults()), "Unsaved: defaults, including every mapping. Save settings to apply."));
}

#[test]
fn detents_are_captured_where_the_lever_is() {
    let mut s = loaded();
    for (field, name) in [(Field::Idle, "Idle"), (Field::Cruise, "CL (cruise)"), (Field::Boost, "Boost")] {
        s.capture_detent(field, 0.123);
        assert_eq!(message(&s).0, format!("Unsaved: {name} captured at 12%."));
        assert_eq!(s.fields[&field], "12");
    }
    assert_eq!(s.draft.as_ref().unwrap().throttle.boost, 0.12);
}

#[test]
fn saving_and_discarding() {
    let mut s = loaded();
    s.type_number(Field::Curve, "25");
    let requests = s.save();
    assert!(matches!(requests.as_slice(), [Request::Save(config)] if config.axes.roll.curve == 0.25));
    assert_eq!((s.saving, message(&s)), (true, ("Saving\u{2026}", Tone::Dirty)));
    assert!(s.save().is_empty());
    s.saved(true);
    assert_eq!((s.saving, s.dirty, message(&s)), (false, false, ("Saved. Applied to the running emulation.", Tone::Saved)));
    s.save();
    s.saved(false);
    assert_eq!(message(&s).0, "Saved.");
    s.save();
    s.save_failed("Disk full.");
    assert_eq!((s.saving, message(&s)), (false, ("Not saved: Disk full.", Tone::Error)));
    s.fields.insert(Field::Curve, "x".into());
    assert!(s.save().is_empty());
    assert!(s.discard(&defaults()).is_empty());
    assert_eq!((s.fields[&Field::Curve].as_str(), message(&s)), ("0", ("Changes discarded.", Tone::Saved)));
}

#[test]
fn try_in_game_sends_one_draft_at_a_time() {
    let mut s = loaded();
    assert!(s.push_preview().is_empty());
    s.fields.insert(Field::Curve, "x".into());
    assert!(s.set_preview(true).is_empty());
    assert!(!s.previewing);
    s.fill();
    assert!(matches!(s.set_preview(true).as_slice(), [Request::Preview(_)]));
    assert!(s.sending);
    // While one is in flight, edits queue a single follow-up with the latest draft.
    assert!(s.type_number(Field::Curve, "10").is_empty());
    assert!(s.type_number(Field::Curve, "20").is_empty());
    assert!(s.queued);
    assert_eq!(message(&s).0, "Unsaved changes \u{b7} running in game now.");
    assert!(matches!(s.previewed().as_slice(), [Request::Preview(config)] if config.axes.roll.curve == 0.2));
    assert!(s.previewed().is_empty());
    assert_eq!(message(&s), ("Unsaved changes \u{b7} running in game now.", Tone::Dirty));
    // The daemon ends it (emulation stopped), or it fails.
    s.preview_gone();
    assert_eq!((s.previewing, message(&s).0), (false, "Unsaved changes \u{b7} the preview ended with emulation."));
    s.preview_gone();
    s.previewed();
    s.set_preview(true);
    s.preview_failed("Not emulating.");
    assert_eq!((s.previewing, s.sending, message(&s)), (false, false, ("Preview stopped: Not emulating.", Tone::Error)));
    s.set_preview(true);
    s.sending = true;
    s.preview_gone();
    assert!(s.previewing);
    s.sending = false;
    // An invalid field while previewing sends nothing until it is fixed.
    assert!(s.type_number(Field::Curve, "500").is_empty());
    assert!(matches!(s.type_number(Field::Curve, "20").as_slice(), [Request::Preview(_)]));
    s.sending = false;
    // Ending it, by switch, by leaving the settings or by discarding.
    assert_eq!(s.set_preview(false), [Request::EndPreview]);
    assert!(s.set_preview(false).is_empty());
    s.preview_end_failed("Gone.");
    assert_eq!(message(&s), ("Preview not ended: Gone.", Tone::Error));
    assert!(s.leave().is_empty());
    s.set_preview(true);
    s.previewed();
    assert_eq!(s.leave(), [Request::EndPreview]);
    assert_eq!(message(&s).0, "Unsaved changes \u{b7} preview ended when you left the settings.");
    s.set_preview(true);
    s.previewed();
    assert_eq!(s.discard(&defaults()), [Request::EndPreview]);
}

#[test]
fn controls_are_picked_by_pressing_them() {
    let mut s = loaded();
    let keys = |list: &[&str]| list.iter().map(|key| key.to_string()).collect::<Vec<_>>();
    assert_eq!(s.single(Kind::Precision), None);
    s.start_capture(Kind::Precision);
    s.start_capture(Kind::Precision);
    assert_eq!(s.capture, None);
    // Picking another kind replaces the pick in progress.
    s.start_capture(Kind::Pause);
    s.start_capture(Kind::Shift);
    assert_eq!(s.capture.as_ref().map(|capture| capture.kind), Some(Kind::Shift));
    s.start_capture(Kind::Shift);
    s.start_capture(Kind::Precision);
    // A hat direction does not count; the first button does.
    assert!(s.press(&keys(&["hat-up"]), &keys(&["hat-up"]), false, 0.0).is_empty());
    s.press(&keys(&["s5", "s6"]), &keys(&["s5", "s6"]), false, 10.0);
    assert_eq!((s.single(Kind::Precision).map(String::as_str), s.capture.clone(), message(&s).0), (Some("s5"), None, "Unsaved: hold Sidestick button 5 for precision aiming."));
    s.start_capture(Kind::Shift);
    // A poll with nothing newly pressed keeps waiting.
    assert!(s.press(&keys(&["s5"]), &[], false, 15.0).is_empty());
    assert_eq!(s.capture.as_ref().map(|capture| capture.kind), Some(Kind::Shift));
    s.press(&keys(&["q2"]), &keys(&["q2"]), false, 20.0);
    assert_eq!((s.single(Kind::Shift).map(String::as_str), message(&s).0), (Some("q2"), "Unsaved: hold Quadrant button 2 for the shift layer."));
    s.clear_single(Kind::Shift);
    assert_eq!((s.single(Kind::Shift), message(&s).0), (None, "Unsaved: shift layer off."));
    s.clear_single(Kind::Precision);
    assert_eq!(message(&s).0, "Unsaved: precision aiming off.");
    // A combo: up to four different controls, then Done.
    assert!(s.record_combo(Kind::Pause).is_empty());
    s.press(&keys(&["s1", "s1", "s2"]), &keys(&["s1", "s1", "s2"]), false, 30.0);
    s.press(&keys(&["s3", "s4", "s5"]), &keys(&["s3", "s4", "s5"]), false, 40.0);
    assert_eq!(s.capture, Some(Capture { kind: Kind::Pause, keys: keys(&["s1", "s2", "s3", "s4"]) }));
    s.record_combo(Kind::Pause);
    assert_eq!((s.draft.as_ref().unwrap().combos.pause.clone(), message(&s).0), (keys(&["s1", "s2", "s3", "s4"]), "Unsaved: new pause combo."));
    // Done with nothing pressed turns it off; another kind's Record starts a new capture.
    s.start_capture(Kind::Next);
    s.record_combo(Kind::Next);
    assert_eq!(message(&s).0, "Unsaved: next-profile combo off.");
    s.start_capture(Kind::Pause);
    s.record_combo(Kind::Next);
    assert_eq!(s.capture.as_ref().map(|capture| capture.kind), Some(Kind::Next));
    s.press(&keys(&["q9"]), &keys(&["q9"]), false, 50.0);
    s.record_combo(Kind::Next);
    assert_eq!(s.draft.as_ref().unwrap().combos.next_profile, keys(&["q9"]));
    assert_eq!(Kind::Next.combo(s.draft.as_ref().unwrap()), &keys(&["q9"]));
    s.clear_combo(Kind::Pause);
    s.clear_combo(Kind::Next);
    assert_eq!((Kind::Pause.combo(s.draft.as_ref().unwrap()).len(), message(&s).0), (0, "Unsaved: next-profile combo off."));
    assert_eq!(Settings::default().single(Kind::Shift), None);
}

#[test]
fn presses_and_the_hat_hold_are_timed() {
    let mut s = loaded();
    let keys = |list: &[&str]| list.iter().map(|key| key.to_string()).collect::<Vec<_>>();
    assert_eq!(s.latest(), None);
    s.press(&keys(&["s1", "hat-up"]), &keys(&["s1", "hat-up"]), true, 100.0);
    assert_eq!(s.hat_since, Some(100.0));
    s.press(&keys(&["hat-up"]), &[], true, 300.0);
    assert_eq!((s.hat_since, s.presses[0].1.end), (Some(100.0), Some(300.0)));
    s.press(&keys(&["s2"]), &keys(&["s2"]), true, 400.0);
    assert_eq!(s.hat_since, None);
    let (key, presses) = s.latest().unwrap();
    assert_eq!((key.as_str(), presses.len()), ("s2", 1));
    // Ended presses older than the timeline drop off; the hat does not count while it is mapped.
    s.press(&keys(&["hat-up"]), &keys(&["hat-up"]), false, 10_000.0);
    assert_eq!((s.presses.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), s.hat_since), (vec!["s2", "hat-up"], None));
}
