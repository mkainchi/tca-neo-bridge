//! The Mapping page's rules: names, mappings per action and layer, changes, feeds and recording into the profile.
use neo_common::config::{Config, Direction, HatMode, Lever, Mode, Target, YawSource};
use neo_config::diagrams::{self, Hand};
use neo_config::mapping::{self, apply, axes, current, describe, diff, label, neo_name, short, sources, sources_for, Layer, Mapping, Neo, Slot, MODES};

fn defaults() -> Config {
    neo_daemon::config::defaults()
}

fn mapping(buttons: &[u8], pov: Option<Direction>, mode: Mode) -> Mapping {
    Mapping { buttons: buttons.to_vec(), pov, mode }
}

#[test]
fn sources_and_targets_have_names() {
    let all = sources();
    assert_eq!((all.len(), all[0].as_str(), all[16].as_str(), all[47].as_str()), (48, "s1", "s17", "q31"));
    assert_eq!(
        ["s3", "s17", "q3", "q9", "q16", "q17", "hat-up"].map(label),
        ["Sidestick button 3", "Sidestick input 17", "Quadrant button 3", "Quadrant detent state 9", "Quadrant detent state 16", "Quadrant add-on input 17", "Sidestick hat up"]
    );
    assert_eq!(["s6", "q2", "hat-left"].map(short), ["S6", "Q2", "Hat left"]);
    assert_eq!((neo_name(Neo::Button(1)), neo_name(Neo::Button(11)), neo_name(Neo::Pov(Direction::Down))), ("NEO 1 (Gun)".into(), "NEO 11".into(), "NEO POV down".into()));
    assert_eq!((1..=15).filter_map(mapping::neo_action).count(), 11);
    assert_eq!(mapping::DIRECTIONS.map(mapping::direction_name), ["up", "right", "down", "left"]);
}

#[test]
fn mappings_describe_toggle_and_compare() {
    assert_eq!(describe(&mapping(&[], None, Mode::Hold)), "Nothing");
    assert_eq!(describe(&mapping(&[1, 11], Some(Direction::Up), Mode::Hold)), "NEO 1 (Gun), NEO 11, NEO POV up");
    assert_eq!(describe(&mapping(&[7], None, Mode::PulseChange)), "NEO 7 (Flares) \u{b7} pulse on change");
    let mut built = mapping(&[5], None, Mode::Hold);
    built.toggle(Neo::Button(2));
    built.toggle(Neo::Pov(Direction::Left));
    assert_eq!((built.buttons.clone(), built.pov), (vec![2, 5], Some(Direction::Left)));
    assert!(built.has(Neo::Button(2)) && built.has(Neo::Pov(Direction::Left)) && !built.has(Neo::Pov(Direction::Up)));
    built.toggle(Neo::Button(5));
    built.toggle(Neo::Pov(Direction::Left));
    assert_eq!(built, mapping(&[2], None, Mode::Hold));
    let before = mapping(&[1, 3], Some(Direction::Up), Mode::Hold);
    let after = mapping(&[3, 4], Some(Direction::Down), Mode::Repeat);
    assert_eq!(
        diff(&before, &after),
        [(true, "Adds NEO 4 (Target select)".into()), (false, "Removes NEO 1 (Gun)".into()), (true, "Adds NEO POV down".into()), (false, "Removes NEO POV up".into()), (true, "Behaviour: Repeat".into())]
    );
    assert_eq!(diff(&mapping(&[], None, Mode::Hold), &mapping(&[], Some(Direction::Up), Mode::Hold)), [(true, "Adds NEO POV up".into())]);
    assert_eq!(diff(&mapping(&[], Some(Direction::Up), Mode::Hold), &mapping(&[], None, Mode::Hold)), [(false, "Removes NEO POV up".into())]);
    assert!(diff(&before, &before).is_empty());
}

#[test]
fn behaviours_and_actions_have_their_texts() {
    for mode in MODES {
        assert!(!mapping::mode_label(mode).is_empty() && mapping::mode_help(mode).ends_with('.'));
        let (title, detail) = mapping::mode_card(mode);
        assert!(!title.is_empty() && !detail.is_empty());
        assert!(mapping::mode_pulses(mode).iter().all(|(start, width)| start + width <= 1.0));
    }
    assert_eq!(Slot::ALL.map(Slot::label), ["Press", "Long press", "Double tap"]);
    assert!(Slot::ALL.iter().all(|slot| slot.help().ends_with('.')));
}

#[test]
fn current_reads_each_action_and_layer() {
    let mut config = defaults();
    config.buttons.insert("s1".into(), vec![9, 1]);
    config.pov.insert("s1".into(), Direction::Up);
    config.behaviours.insert("s1".into(), Mode::Toggle);
    config.shifted.buttons.insert("s1".into(), vec![3]);
    config.shifted.pov.insert("s1".into(), Direction::Down);
    config.long_press.insert("s1".into(), Target { buttons: vec![8, 2], pov: Some(Direction::Left) });
    config.double_tap.insert("s1".into(), Target { buttons: vec![4], pov: None });
    assert_eq!(current(&config, "s1", Slot::Press, Layer::Normal), mapping(&[1, 9], Some(Direction::Up), Mode::Toggle));
    assert_eq!(current(&config, "s1", Slot::Press, Layer::Shift), mapping(&[3], Some(Direction::Down), Mode::Toggle));
    assert_eq!(current(&config, "s1", Slot::Long, Layer::Shift), mapping(&[2, 8], Some(Direction::Left), Mode::Hold));
    assert_eq!(current(&config, "s1", Slot::Double, Layer::Normal), mapping(&[4], None, Mode::Hold));
    assert_eq!(current(&config, "q20", Slot::Press, Layer::Normal), mapping(&[], None, Mode::Hold));
    assert_eq!(current(&config, "q20", Slot::Long, Layer::Normal), mapping(&[], None, Mode::Hold));
}

#[test]
fn feeds_follow_the_normal_press() {
    let mut config = defaults();
    config.pov.insert("hat-up".into(), Direction::Up);
    config.pov.insert("s16".into(), Direction::Up);
    assert_eq!(sources_for(&config, Neo::Button(5)), ["s5", "q1", "q2"]);
    assert_eq!(sources_for(&config, Neo::Pov(Direction::Up)), ["s16", "hat-up"]);
    assert!(sources_for(&config, Neo::Pov(Direction::Down)).is_empty());
}

#[test]
fn apply_writes_one_action_and_removes_empty_ones() {
    let config = defaults();
    let next = apply(&config, "q20", Slot::Press, Layer::Normal, &mapping(&[2], Some(Direction::Right), Mode::Pulse));
    assert_eq!((next.buttons["q20"].clone(), next.pov["q20"], next.behaviours["q20"]), (vec![2], Direction::Right, Mode::Pulse));
    let cleared = apply(&next, "q20", Slot::Press, Layer::Normal, &mapping(&[], None, Mode::Hold));
    assert!(!cleared.buttons.contains_key("q20") && !cleared.pov.contains_key("q20") && !cleared.behaviours.contains_key("q20"));
    let shifted = apply(&config, "s1", Slot::Press, Layer::Shift, &mapping(&[4], None, Mode::Hold));
    assert_eq!((shifted.shifted.buttons["s1"].clone(), shifted.buttons["s1"].clone()), (vec![4], vec![1]));
    let long = apply(&config, "s1", Slot::Long, Layer::Normal, &mapping(&[], Some(Direction::Up), Mode::Hold));
    assert_eq!(long.long_press["s1"], Target { buttons: vec![], pov: Some(Direction::Up) });
    let double = apply(&config, "s1", Slot::Double, Layer::Normal, &mapping(&[6], None, Mode::Hold));
    assert_eq!(double.double_tap["s1"], Target { buttons: vec![6], pov: None });
    assert!(apply(&double, "s1", Slot::Double, Layer::Normal, &mapping(&[], None, Mode::Hold)).double_tap.is_empty());
    assert_eq!(neo_daemon::config::validate(&next), Ok(()));
}

#[test]
fn axes_name_their_sources() {
    let mut config = defaults();
    assert_eq!(axes(&config).map(|row| row.1), ["Sidestick stick", "Sidestick twist", "Left quadrant lever or sidestick slider", "Sidestick hat"].map(String::from));
    config.throttle_lever = Lever::Right;
    config.yaw_source = YawSource::Off;
    config.hat = HatMode::Mapped;
    config.camera.lever = true;
    assert_eq!(axes(&config).map(|row| row.1)[1..], ["No source (yaw off)", "Right quadrant lever or sidestick slider", "No source (hat is mapped) + spare lever"].map(String::from));
    config.yaw_source = YawSource::LeverDifferential;
    assert_eq!(axes(&config)[1].1, "Quadrant lever difference");
    assert_eq!(axes(&config)[2].1, "Average of both quadrant levers or sidestick slider");
}

#[test]
fn diagrams_place_hotspots_in_their_own_units() {
    let plate = eframe::egui::Rect::from_min_size(eframe::egui::pos2(100.0, 50.0), eframe::egui::vec2(390.0, 340.0));
    let (centre, size) = diagrams::RIGHT.place(plate, (-76.0, -8.0, 27.0));
    assert_eq!(centre, eframe::egui::pos2(100.0, 50.0));
    assert!((size - 27.0).abs() < 1e-4);
    let small = eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::vec2(195.0, 170.0));
    assert_eq!(diagrams::RIGHT.place(small, (0.0, 0.0, 27.0)).1, 18.0);
    assert!((diagrams::NEO.aspect() - 606.0 / 726.0).abs() < 1e-6);
    assert!(std::ptr::eq(diagrams::stick(Hand::Left), &diagrams::LEFT) && std::ptr::eq(diagrams::stick(Hand::Right), &diagrams::RIGHT));
    let targets: Vec<Option<Neo>> = diagrams::NEO.spots.iter().map(|spot| diagrams::neo_target(spot.0)).collect();
    assert_eq!(targets.iter().filter(|target| target.is_none()).count(), 1);
    assert_eq!(targets[16..], [Some(Neo::Pov(Direction::Up)), Some(Neo::Pov(Direction::Right)), Some(Neo::Pov(Direction::Down)), Some(Neo::Pov(Direction::Left))]);
    // Every hotspot names a real source.
    for diagram in [&diagrams::LEFT, &diagrams::RIGHT, &diagrams::QUADRANT] {
        assert!(diagram.spots.iter().all(|spot| sources().iter().any(|key| key == spot.0)));
    }
}
