//! The printable cheat sheet: every mapped control, the axes, the helpers and the timing, escaped.
use neo_common::config::{Config, Direction, HatMode, Lever, Mode, Target, ThrottleMode, YawSource};
use neo_config::sheet::{body, date, escape, page};

fn defaults() -> Config {
    neo_daemon::config::defaults()
}

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
    let mut config = defaults();
    config.precision.source = None;
    config.shift = None;
    config.combos.pause.clear();
    config.combos.next_profile.clear();
    config.split_high_g.enabled = false;
    config.hat = HatMode::Camera;
    config.camera.full_after_hold = false;
    config.throttle_lever = Lever::Left;
    config.yaw_source = YawSource::Twist;
    config.throttle.mode = ThrottleMode::Linear;
    config.axes.roll.curve = 0.0;
    config.axes.yaw.curve = 0.0;
    let sheet = body(&config, None, "1 Jan 1970");
    for part in ["<p>Profile: Unsaved profile</p>", "Printed 1 Jan 1970", "<td>Roll / pitch</td>", "<td class=\"k\">Twist</td><td>Yaw</td>", "<td class=\"k\">Left lever</td><td>Throttle</td>", "<td>Camera</td>", "<p>None set.</p>"] {
        assert!(sheet.contains(part), "no {part} in {sheet}");
    }
    assert!(!sheet.contains("Shift (hold"));
    let full = page(&config, Some("A & B"), "x");
    assert!(full.starts_with("<!doctype html>") && full.contains("onload=\"print()\"") && full.contains("Profile: A &amp; B"));
}

#[test]
fn every_gesture_layer_and_helper_is_printed() {
    let mut config = defaults();
    config.buttons.clear();
    config.pov.clear();
    config.behaviours.clear();
    config.buttons.insert("s1".into(), vec![1]);
    config.behaviours.insert("s1".into(), Mode::Toggle);
    config.buttons.insert("s2".into(), vec![16]);
    config.behaviours.insert("s2".into(), Mode::Hold);
    config.pov.insert("s3".into(), Direction::Left);
    config.behaviours.insert("s4".into(), Mode::Toggle);
    config.long_press.insert("s4".into(), Target { buttons: vec![2], pov: None });
    config.double_tap.insert("s5".into(), Target { buttons: vec![], pov: Some(Direction::Up) });
    config.buttons.insert("s6".into(), vec![3]);
    config.shift = Some("s6".into());
    config.shifted.buttons.insert("s1".into(), vec![4]);
    config.hat = HatMode::Mapped;
    config.pov.insert("hat-up".into(), Direction::Up);
    config.precision.source = Some("q1".into());
    config.precision.scale = 0.5;
    config.combos.pause = vec!["s1".into(), "q2".into()];
    config.combos.next_profile = vec!["q3".into()];
    config.split_high_g.enabled = true;
    config.throttle_lever = Lever::Right;
    config.throttle.mode = ThrottleMode::Detents;
    config.yaw_source = YawSource::LeverDifferential;
    config.axes.roll.curve = 0.25;
    config.axes.yaw.curve = 0.5;
    let sheet = body(&config, Some("Dogfight"), "d");
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
        "Roll / pitch \u{b7} curve 25%",
        "<td class=\"k\">Lever difference</td><td>Yaw \u{b7} curve 50%</td>",
        "<td class=\"k\">Both levers</td><td>Throttle \u{b7} detents",
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
    let sheet = body(&config, None, "d");
    assert!(sheet.contains("<td class=\"k\">Right lever</td>") && sheet.contains("<td>Yaw off</td>") && sheet.contains(&format!("Camera \u{b7} full after {} ms hold", config.timing.long_ms)));
    assert!(sheet.contains("<td class=\"k\">Twist</td>"));
}