//! The Mapping page's rules: names, sources per controller, mappings per action and layer, changes, feeds and
//! recording into the profile.
mod common;
use common::{t16000m, tca, twcs, view};
use neo_common::config::{Binding, Direction, HatMode, Mode, Target, YawSource};
use neo_config::diagrams::{self, Hand};
use neo_config::mapping::{self, apply, axes, current, describe, diff, neo_name, short, sources_for, Controllers, Layer, Mapping, Neo, Slot, MODES};

fn mapping(buttons: &[u8], pov: Option<Direction>, mode: Mode) -> Mapping {
    Mapping { buttons: buttons.to_vec(), pov, mode }
}

/// A T.16000M stick and a TWCS throttle, both taking part.
fn generic() -> Controllers {
    Controllers { present: vec![t16000m(), twcs()], active: vec!["t16000m".into(), "twcs".into()] }
}

#[test]
fn tca_sources_and_targets_keep_their_names() {
    let all = tca().sources();
    assert_eq!(
        (all.len(), all[0].as_str(), all[30].as_str(), all[31].as_str(), all[47].as_str(), all[51].as_str()),
        (52, "tca-quadrant/b1", "tca-quadrant/b31", "tca-stick/b1", "tca-stick/b17", "tca-stick/hat-left")
    );
    let tca = tca();
    assert_eq!(
        ["tca-stick/b3", "tca-stick/b17", "tca-quadrant/b3", "tca-quadrant/b9", "tca-quadrant/b16", "tca-quadrant/b17", "tca-stick/hat-up"].map(|key| tca.label(key)),
        ["Sidestick button 3", "Sidestick input 17", "Quadrant button 3", "Quadrant detent state 9", "Quadrant detent state 16", "Quadrant add-on input 17", "Sidestick hat up"]
    );
    assert_eq!(["tca-stick/b6", "tca-quadrant/b2", "tca-stick/hat-left"].map(short), ["S6", "Q2", "Hat left"]);
    assert_eq!((neo_name(Neo::Button(1)), neo_name(Neo::Button(11)), neo_name(Neo::Pov(Direction::Down))), ("NEO 1 (Gun)".into(), "NEO 11".into(), "NEO POV down".into()));
    assert_eq!((1..=15).filter_map(mapping::neo_action).count(), 11);
    assert_eq!(mapping::DIRECTIONS.map(mapping::direction_name), ["up", "right", "down", "left"]);
}

#[test]
fn any_controller_is_named_by_its_device() {
    let generic = generic();
    let all = generic.sources();
    assert_eq!((all.len(), all[0].as_str(), all[19].as_str(), all[20].as_str(), all[37].as_str()), (38, "t16000m/b1", "t16000m/hat-left", "twcs/b1", "twcs/hat-left"));
    assert_eq!(["t16000m/b3", "twcs/hat-up", "gone/b2"].map(|key| generic.label(key)), ["T.16000M button 3", "TWCS Throttle hat up", "gone button 2"]);
    assert_eq!(["twcs/b3", "twcs/hat-up", "b4"].map(short), ["twcs 3", "twcs hat up", " 4"]);
    assert_eq!((mapping::parts("twcs/b3"), mapping::parts("b3"), mapping::hat_direction("twcs/hat-down"), mapping::hat_direction("twcs/b3")), (("twcs", "b3"), ("", "b3"), Some("down"), None));
    assert_eq!((generic.name("twcs"), generic.name("gone")), ("TWCS Throttle", "gone"));
    assert_eq!((generic.binding(&Binding::new("twcs", "z", true)), generic.binding(&Binding::new("t16000m", "x", false))), ("TWCS Throttle z (inverted)".into(), "T.16000M x".into()));
    // Only the controllers taking part give sources.
    let stick_only = Controllers { active: vec!["t16000m".into(), "unplugged".into()], ..generic };
    assert_eq!(stick_only.sources().len(), 20);
}

#[test]
fn the_camera_hat_is_the_chosen_one_or_the_first() {
    let generic = generic();
    let mut config = view();
    config.camera_hat = Some("twcs".into());
    assert_eq!(generic.camera(&config), Some("twcs"));
    config.camera_hat = Some("tca-stick".into());
    assert_eq!(generic.camera(&config), Some("t16000m"));
    assert_eq!(Controllers::default().camera(&config), None);
    // A spare axis for a new camera lever: neither a flight axis nor a throttle.
    config.bindings.roll = Some(Binding::new("t16000m", "x", false));
    config.bindings.pitch = Some(Binding::new("t16000m", "y", false));
    config.bindings.yaw = None;
    config.bindings.throttle = vec![];
    assert_eq!(generic.spare_axis(&config), Some(Binding::new("t16000m", "rz", false)));
    assert_eq!(generic.axes().len(), 9);
    assert_eq!(Controllers::default().spare_axis(&config), None);
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
    let key = "tca-stick/b1";
    let mut config = view();
    config.buttons.insert(key.into(), vec![9, 1]);
    config.pov.insert(key.into(), Direction::Up);
    config.behaviours.insert(key.into(), Mode::Toggle);
    config.shifted.buttons.insert(key.into(), vec![3]);
    config.shifted.pov.insert(key.into(), Direction::Down);
    config.long_press.insert(key.into(), Target { buttons: vec![8, 2], pov: Some(Direction::Left) });
    config.double_tap.insert(key.into(), Target { buttons: vec![4], pov: None });
    assert_eq!(current(&config, key, Slot::Press, Layer::Normal), mapping(&[1, 9], Some(Direction::Up), Mode::Toggle));
    assert_eq!(current(&config, key, Slot::Press, Layer::Shift), mapping(&[3], Some(Direction::Down), Mode::Toggle));
    assert_eq!(current(&config, key, Slot::Long, Layer::Shift), mapping(&[2, 8], Some(Direction::Left), Mode::Hold));
    assert_eq!(current(&config, key, Slot::Double, Layer::Normal), mapping(&[4], None, Mode::Hold));
    assert_eq!(current(&config, "tca-quadrant/b20", Slot::Press, Layer::Normal), mapping(&[], None, Mode::Hold));
    assert_eq!(current(&config, "tca-quadrant/b20", Slot::Long, Layer::Normal), mapping(&[], None, Mode::Hold));
}

#[test]
fn feeds_follow_the_normal_press_of_the_controllers_taking_part() {
    let mut config = view();
    config.pov.insert("tca-stick/hat-up".into(), Direction::Up);
    config.pov.insert("tca-stick/b16".into(), Direction::Up);
    config.pov.insert("twcs/b1".into(), Direction::Up);
    assert_eq!(sources_for(&config, &tca(), Neo::Button(5)), ["tca-quadrant/b1", "tca-quadrant/b2", "tca-stick/b5"]);
    assert_eq!(sources_for(&config, &tca(), Neo::Pov(Direction::Up)), ["tca-stick/b16", "tca-stick/hat-up"]);
    assert!(sources_for(&config, &tca(), Neo::Pov(Direction::Down)).is_empty());
}

#[test]
fn apply_writes_one_action_and_removes_empty_ones() {
    let config = view();
    let key = "tca-quadrant/b20";
    let next = apply(&config, key, Slot::Press, Layer::Normal, &mapping(&[2], Some(Direction::Right), Mode::Pulse));
    assert_eq!((next.buttons[key].clone(), next.pov[key], next.behaviours[key]), (vec![2], Direction::Right, Mode::Pulse));
    let cleared = apply(&next, key, Slot::Press, Layer::Normal, &mapping(&[], None, Mode::Hold));
    assert!(!cleared.buttons.contains_key(key) && !cleared.pov.contains_key(key) && !cleared.behaviours.contains_key(key));
    let stick = "tca-stick/b1";
    let shifted = apply(&config, stick, Slot::Press, Layer::Shift, &mapping(&[4], None, Mode::Hold));
    assert_eq!((shifted.shifted.buttons[stick].clone(), shifted.buttons[stick].clone()), (vec![4], vec![1]));
    let long = apply(&config, stick, Slot::Long, Layer::Normal, &mapping(&[], Some(Direction::Up), Mode::Hold));
    assert_eq!(long.long_press[stick], Target { buttons: vec![], pov: Some(Direction::Up) });
    let double = apply(&config, stick, Slot::Double, Layer::Normal, &mapping(&[6], None, Mode::Hold));
    assert_eq!(double.double_tap[stick], Target { buttons: vec![6], pov: None });
    assert!(apply(&double, stick, Slot::Double, Layer::Normal, &mapping(&[], None, Mode::Hold)).double_tap.is_empty());
    assert_eq!(neo_daemon::config::validate(&next.compose()), Ok(()));
}

#[test]
fn axes_name_their_bindings() {
    let mut config = view();
    let tca = tca();
    assert_eq!(
        axes(&config, &tca).map(|row| row.1),
        ["TCA Sidestick Airbus x", "TCA Sidestick Airbus y", "TCA Sidestick Airbus rz", "TCA Sidestick Airbus slider or TCA Quadrant Eng 1&2 left", "TCA Sidestick Airbus hat"].map(String::from)
    );
    config.yaw_source = YawSource::Off;
    config.hat = HatMode::Mapped;
    config.camera_lever = Some(Binding::new("tca-quadrant", "right", true));
    config.bindings.roll = None;
    config.bindings.throttle.clear();
    assert_eq!(axes(&config, &tca).map(|row| row.1)[..], ["No source", "TCA Sidestick Airbus y", "No source (yaw off)", "No source", "No source (hat is mapped) + TCA Quadrant Eng 1&2 right (inverted)"].map(String::from));
    config.yaw_source = YawSource::LeverDifferential;
    config.hat = HatMode::Camera;
    assert_eq!(axes(&config, &tca)[2].1, "Split lever difference");
    // Without controllers there is no camera hat, and bindings fall back to device ids.
    assert_eq!(axes(&config, &Controllers::default())[4].1, "No hat + tca-quadrant right (inverted)");
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
    // Every hotspot names a real source of the TCA pair.
    let sources = tca().sources();
    for diagram in [&diagrams::LEFT, &diagrams::RIGHT, &diagrams::QUADRANT] {
        assert!(diagram.spots.iter().all(|spot| sources.iter().any(|key| key == spot.0)));
    }
}
