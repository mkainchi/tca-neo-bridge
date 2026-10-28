//! The printable cheat sheet of one controller set: every mapped control, the axes, the helpers and the timing,
//! escaped.
mod common;
use common::{t16000m, tca, view};
use neo_common::config::{Binding, Direction, HatMode, Mode, Target, ThrottleMode, YawSource};
use neo_config::mapping::Controllers;
use neo_config::sheet::{body, date, escape, page};

#[test]
fn values_are_escaped() {
    assert_eq!(escape(r#"<a href="x">&</a>"#), "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;");
}

#[test]
fn dates_read_as_the_page_printed_them() {
    assert_eq!([0, 59, 60, 11_016, 20_753, -1].map(date), ["1 Jan 1970", "1 Mar 1970", "2 Mar 1970", "29 Feb 2000", "27 Oct 2026", "31 Dec 1969"]);
}

#[test]
fn the_default_profile_lists_its_buttons_axes_and_no_helpers() {
    let mut config = view();
    config.precision.source = None;
    config.shift = None;
    config.combos.pause.clear();
    config.combos.next_profile.clear();
    config.split_high_g.enabled = false;
    config.hat = HatMode::Camera;
    config.camera.full_after_hold = false;
    config.yaw_source = YawSource::Twist;
    config.throttle.mode = ThrottleMode::Linear;
    config.axes.roll.curve = 0.0;
    config.axes.yaw.curve = 0.0;
    let sheet = body(&config, &tca(), None, "1 Jan 1970");
    for part in [
        "<h1>TCA Quadrant Eng 1&amp;2 + TCA Sidestick Airbus \u{2192} ACE COMBAT 8</h1>",
        "<p>Profile: Unsaved profile</p>",
        "Any-TM-HOTAS bridge<br>Printed 1 Jan 1970",
        "<th>Control</th>",
        "<td class=\"k\">TCA Sidestick Airbus x / TCA Sidestick Airbus y</td><td>Roll / pitch</td>",
        "<td class=\"k\">TCA Sidestick Airbus rz</td><td>Yaw</td>",
        "<td class=\"k\">TCA Sidestick Airbus slider or TCA Quadrant Eng 1&amp;2 left</td><td>Throttle</td>",
        "<td class=\"k\">TCA Sidestick Airbus hat</td><td>Camera</td>",
        "<p>None set.</p>",
    ] {
        assert!(sheet.contains(part), "no {part} in {sheet}");
    }
    // The camera hat's directions are not buttons while it drives the camera.
    assert!(!sheet.contains("Shift (hold") && !sheet.contains("Hat up"));
    let full = page(&config, &tca(), Some("A & B"), "x");
    assert!(full.starts_with("<!doctype html>") && full.contains("onload=\"print()\"") && full.contains("Profile: A &amp; B") && full.contains("<title>Any-TM-HOTAS bridge cheat sheet</title>"));
}

#[test]
fn every_gesture_layer_and_helper_is_printed() {
    let (s, q) = (|n: u8| format!("tca-stick/b{n}"), |n: u8| format!("tca-quadrant/b{n}"));
    let mut config = view();
    config.buttons.clear();
    config.pov.clear();
    config.behaviours.clear();
    config.buttons.insert(s(1), vec![1]);
    config.behaviours.insert(s(1), Mode::Toggle);
    config.buttons.insert(s(2), vec![16]);
    config.behaviours.insert(s(2), Mode::Hold);
    config.pov.insert(s(3), Direction::Left);
    config.behaviours.insert(s(4), Mode::Toggle);
    config.long_press.insert(s(4), Target { buttons: vec![2], pov: None });
    config.double_tap.insert(s(5), Target { buttons: vec![], pov: Some(Direction::Up) });
    config.buttons.insert(s(6), vec![3]);
    config.shift = Some(s(6));
    config.shifted.buttons.insert(s(1), vec![4]);
    config.hat = HatMode::Mapped;
    config.pov.insert("tca-stick/hat-up".into(), Direction::Up);
    config.precision.source = Some(q(1));
    config.precision.scale = 0.5;
    config.combos.pause = vec![s(1), q(2)];
    config.combos.next_profile = vec![q(3)];
    config.split_high_g.enabled = true;
    config.throttle.mode = ThrottleMode::Detents;
    config.yaw_source = YawSource::LeverDifferential;
    config.axes.roll.curve = 0.25;
    config.axes.yaw.curve = 0.5;
    config.bindings.roll = None;
    config.bindings.throttle.clear();
    let sheet = body(&config, &tca(), Some("Dogfight"), "d");
    let row = |key: &str| sheet.split("<tr>").find(|row| row.starts_with(&format!("<td class=\"k\">{key}</td>"))).map(str::to_string);
    let s1 = row("S1").unwrap();
    assert!(s1.contains("<span class=\"tag\">toggle</span>") && !s1.contains("same"), "{s1}");
    assert!(!row("S2").unwrap().contains("tag"));
    assert!(row("S3").unwrap().contains("POV left"));
    // A long press alone: no press, no tag.
    let s4 = row("S4").unwrap();
    assert!(s4.starts_with("<td class=\"k\">S4</td><td class=\"x\">\u{2014}</td>") && !s4.contains("tag"), "{s4}");
    assert!(row("S5").unwrap().contains("POV up"));
    assert!(row("S6").is_none() && row("S7").is_none());
    assert!(row("Hat up").unwrap().contains("POV up"));
    assert!(row("S2").unwrap().contains("<td class=\"x\">same</td>"));
    for part in [
        "<th>Shift (hold S6)</th>",
        "Profile: Dogfight",
        "<td class=\"k\">No source / TCA Sidestick Airbus y</td><td>Roll / pitch \u{b7} curve 25%",
        "<td class=\"k\">Lever difference</td><td>Yaw \u{b7} curve 50%</td>",
        "<td class=\"k\">No source</td><td>Throttle \u{b7} detents",
        "Mapped as buttons (see above)",
        "Precision aim \u{b7} 50%",
        "Hold S6</td><td>Shift layer",
        "S1 + Q2</td><td>Pause / resume input",
        "Q3</td><td>Next profile",
        "Levers split</td><td>High-G",
    ] {
        assert!(sheet.contains(part), "no {part} in {sheet}");
    }
    config.yaw_source = YawSource::Off;
    config.hat = HatMode::Camera;
    config.camera.full_after_hold = true;
    config.bindings.yaw = Some(Binding::new("tca-stick", "rz", true));
    let sheet = body(&config, &tca(), None, "d");
    assert!(sheet.contains("<td>Yaw off</td>") && sheet.contains(&format!("Camera \u{b7} full after {} ms hold", config.timing.long_ms)));
    // A binding is named without its inversion.
    assert!(sheet.contains("<td class=\"k\">TCA Sidestick Airbus rz</td>"));
}

#[test]
fn any_controller_set_gets_a_sheet_of_its_own() {
    let mut config = view();
    config.buttons.insert("t16000m/b3".into(), vec![2]);
    config.pov.insert("t16000m/hat-down".into(), Direction::Down);
    config.hat = HatMode::Mapped;
    let stick = Controllers { present: vec![t16000m()], active: vec!["t16000m".into()] };
    let sheet = body(&config, &stick, None, "d");
    assert!(sheet.contains("<h1>T.16000M \u{2192} ACE COMBAT 8</h1>") && sheet.contains("<td class=\"k\">t16000m 3</td>") && sheet.contains("<td class=\"k\">t16000m hat down</td>"), "{sheet}");
    // No controllers, no hat.
    let none = body(&config, &Controllers::default(), None, "d");
    assert!(none.contains("<h1>No controllers \u{2192} ACE COMBAT 8</h1>") && none.contains("<td class=\"k\">No hat</td>"), "{none}");
}
