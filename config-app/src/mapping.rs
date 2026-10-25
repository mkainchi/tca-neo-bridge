//! The Mapping page's rules, apart from drawing: names for TCA sources and NEO targets, a source's mapping per action
//! and layer, what a change adds and removes, and how a recorded mapping lands in the profile.
use neo_common::config::{Config, Direction, Mode, Target};

/// Sidestick buttons s1..s17 and quadrant inputs q1..q31.
pub const COUNTS: [(char, u8); 2] = [('s', 17), ('q', 31)];

/// Sidestick hat directions; mappable only while the hat is set to Mapped.
pub const HAT_SOURCES: [&str; 4] = ["hat-up", "hat-right", "hat-down", "hat-left"];

/// ACE COMBAT 8 actions on the native NEO buttons (official PC mapping); unlisted buttons have none.
pub fn neo_action(button: u8) -> Option<&'static str> {
    Some(match button {
        1 => "Gun",
        2 => "Missile",
        3 => "Weapon select",
        4 => "Target select",
        5 => "High-G",
        6 => "Autopilot",
        7 => "Flares",
        8 => "View",
        9 => "Radar",
        10 => "Gear",
        12 => "Pause / menu",
        _ => return None,
    })
}

/// Every button source, sidestick first.
pub fn sources() -> Vec<String> {
    COUNTS.iter().flat_map(|(prefix, count)| (1..=*count).map(move |n| format!("{prefix}{n}"))).collect()
}

fn number(key: &str) -> u8 {
    key[1..].parse().expect("a source key")
}

/// "Sidestick button 3", "Quadrant detent state 9", "Sidestick hat up".
pub fn label(key: &str) -> String {
    if let Some(direction) = key.strip_prefix("hat-") {
        return format!("Sidestick hat {direction}");
    }
    let n = number(key);
    match (key.as_bytes()[0], n) {
        (b's', 17) => "Sidestick input 17".into(),
        (b's', _) => format!("Sidestick button {n}"),
        (_, 17..) => format!("Quadrant add-on input {n}"),
        (_, 9..) => format!("Quadrant detent state {n}"),
        _ => format!("Quadrant button {n}"),
    }
}

/// Compact source name for tables: "S6", "Q2", "Hat up".
pub fn short(key: &str) -> String {
    match key.strip_prefix("hat-") {
        Some(direction) => format!("Hat {direction}"),
        None => key.to_uppercase(),
    }
}

/// A NEO target: one of its buttons, or a POV direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Neo {
    Button(u8),
    Pov(Direction),
}

pub const DIRECTIONS: [Direction; 4] = [Direction::Up, Direction::Right, Direction::Down, Direction::Left];

pub fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "up",
        Direction::Right => "right",
        Direction::Down => "down",
        Direction::Left => "left",
    }
}

pub fn neo_name(neo: Neo) -> String {
    match neo {
        Neo::Pov(direction) => format!("NEO POV {}", direction_name(direction)),
        Neo::Button(n) => neo_action(n).map_or(format!("NEO {n}"), |action| format!("NEO {n} ({action})")),
    }
}

/// What one action of a source sends: NEO buttons (sorted), a POV direction, and (for the press) its behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub buttons: Vec<u8>,
    pub pov: Option<Direction>,
    pub mode: Mode,
}

impl Mapping {
    pub fn has(&self, neo: Neo) -> bool {
        match neo {
            Neo::Button(n) => self.buttons.contains(&n),
            Neo::Pov(direction) => self.pov == Some(direction),
        }
    }

    /// Add or remove a target; one POV direction at most.
    pub fn toggle(&mut self, neo: Neo) {
        match neo {
            Neo::Pov(direction) => self.pov = if self.pov == Some(direction) { None } else { Some(direction) },
            Neo::Button(n) if self.buttons.contains(&n) => self.buttons.retain(|b| *b != n),
            Neo::Button(n) => {
                self.buttons.push(n);
                self.buttons.sort_unstable();
            }
        }
    }
}

pub fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Hold => "Hold",
        Mode::Pulse => "Pulse",
        Mode::PulseChange => "Pulse on change",
        Mode::Toggle => "Toggle",
        Mode::Repeat => "Repeat",
    }
}

/// The behaviour picker's short name and what the game receives.
pub fn mode_card(mode: Mode) -> (&'static str, &'static str) {
    match mode {
        Mode::Hold => ("Hold", "While held"),
        Mode::Pulse => ("Pulse", "On switch-on"),
        Mode::PulseChange => ("On change", "On and off"),
        Mode::Toggle => ("Toggle", "Each press flips"),
        Mode::Repeat => ("Repeat", "While held"),
    }
}

pub const MODES: [Mode; 5] = [Mode::Hold, Mode::Pulse, Mode::PulseChange, Mode::Toggle, Mode::Repeat];

pub fn mode_help(mode: Mode) -> &'static str {
    match mode {
        Mode::Hold => "The NEO button stays pressed while the control is on. A latched switch keeps it held.",
        Mode::Pulse => "One short press when the control turns on. Good for the mode selector: one flare burst, not a stream.",
        Mode::PulseChange => "One short press each time the control moves, on and off. Turning ENG 1 on and off toggles Autopilot on and off.",
        Mode::Toggle => "Each press turns the NEO button on or off, like a latch for a momentary button.",
        Mode::Repeat => "Short presses again and again while held, at the Timing interval: rapid flares or gun bursts.",
    }
}

/// The NEO pulses on the behaviour timeline, as (start, width) fractions of the lane; the control is on 15–70 %.
pub fn mode_pulses(mode: Mode) -> &'static [(f32, f32)] {
    match mode {
        Mode::Hold => &[(0.15, 0.55)],
        Mode::Pulse => &[(0.15, 0.06)],
        Mode::PulseChange => &[(0.15, 0.06), (0.70, 0.06)],
        Mode::Toggle => &[(0.15, 0.85)],
        Mode::Repeat => &[(0.15, 0.06), (0.30, 0.06), (0.45, 0.06), (0.60, 0.06)],
    }
}

/// `{buttons, pov, mode}` → "NEO 1 (Gun), NEO POV up · pulse".
pub fn describe(mapping: &Mapping) -> String {
    let parts: Vec<String> = mapping.buttons.iter().map(|n| neo_name(Neo::Button(*n))).chain(mapping.pov.map(|d| neo_name(Neo::Pov(d)))).collect();
    let text = if parts.is_empty() { "Nothing".to_string() } else { parts.join(", ") };
    match mapping.mode {
        Mode::Hold => text,
        mode => format!("{text} · {}", mode_label(mode).to_lowercase()),
    }
}

/// The actions a source can have: its press (by behaviour), a long press and a double tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Press,
    Long,
    Double,
}

impl Slot {
    pub const ALL: [Slot; 3] = [Slot::Press, Slot::Long, Slot::Double];

    pub fn label(self) -> &'static str {
        match self {
            Slot::Press => "Press",
            Slot::Long => "Long press",
            Slot::Double => "Double tap",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Slot::Press => "Tap or hold, by its behaviour.",
            Slot::Long => "Held for the long-press time: these targets are held until release, instead of the press.",
            Slot::Double => "Two taps within the double-tap window: one short press of these targets.",
        }
    }
}

/// The mapping layer being edited: the normal one, or the one used while the shift control is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Normal,
    Shift,
}

fn sorted(buttons: Option<&Vec<u8>>) -> Vec<u8> {
    let mut buttons = buttons.cloned().unwrap_or_default();
    buttons.sort_unstable();
    buttons
}

/// A source's mapping for one action and layer. Behaviours belong to the press; long press and double tap are the
/// same in both layers.
pub fn current(config: &Config, key: &str, slot: Slot, layer: Layer) -> Mapping {
    let gesture = |targets: &std::collections::BTreeMap<String, Target>| {
        let target = targets.get(key);
        Mapping { buttons: sorted(target.map(|t| &t.buttons)), pov: target.and_then(|t| t.pov), mode: Mode::Hold }
    };
    match (slot, layer) {
        (Slot::Long, _) => gesture(&config.long_press),
        (Slot::Double, _) => gesture(&config.double_tap),
        (Slot::Press, Layer::Normal) => Mapping { buttons: sorted(config.buttons.get(key)), pov: config.pov.get(key).copied(), mode: behaviour(config, key) },
        (Slot::Press, Layer::Shift) => Mapping { buttons: sorted(config.shifted.buttons.get(key)), pov: config.shifted.pov.get(key).copied(), mode: behaviour(config, key) },
    }
}

fn behaviour(config: &Config, key: &str) -> Mode {
    config.behaviours.get(key).copied().unwrap_or_default()
}

/// One line of what a proposal changes: added (true) or removed, and the text.
pub fn diff(before: &Mapping, after: &Mapping) -> Vec<(bool, String)> {
    let mut changes: Vec<(bool, String)> = Vec::new();
    changes.extend(after.buttons.iter().filter(|n| !before.buttons.contains(n)).map(|n| (true, format!("Adds {}", neo_name(Neo::Button(*n))))));
    changes.extend(before.buttons.iter().filter(|n| !after.buttons.contains(n)).map(|n| (false, format!("Removes {}", neo_name(Neo::Button(*n))))));
    if after.pov != before.pov {
        changes.extend(after.pov.map(|d| (true, format!("Adds {}", neo_name(Neo::Pov(d))))));
        changes.extend(before.pov.map(|d| (false, format!("Removes {}", neo_name(Neo::Pov(d))))));
    }
    if after.mode != before.mode {
        changes.push((true, format!("Behaviour: {}", mode_label(after.mode))));
    }
    changes
}

/// TCA sources (buttons and hat directions) whose normal press feeds a NEO target.
pub fn sources_for(config: &Config, neo: Neo) -> Vec<String> {
    sources().into_iter().chain(HAT_SOURCES.map(String::from)).filter(|key| match neo {
        Neo::Pov(direction) => config.pov.get(key) == Some(&direction),
        Neo::Button(n) => config.buttons.get(key).is_some_and(|buttons| buttons.contains(&n)),
    })
    .collect()
}

/// The profile with one action of a source replaced. The press writes the edited layer and the behaviour (shared by
/// both layers); an empty mapping removes the entry.
pub fn apply(config: &Config, key: &str, slot: Slot, layer: Layer, mapping: &Mapping) -> Config {
    let mut next = config.clone();
    let key = key.to_string();
    match slot {
        Slot::Press => {
            let (buttons, pov) = match layer {
                Layer::Normal => (&mut next.buttons, &mut next.pov),
                Layer::Shift => (&mut next.shifted.buttons, &mut next.shifted.pov),
            };
            set(buttons, &key, (!mapping.buttons.is_empty()).then(|| mapping.buttons.clone()));
            set(pov, &key, mapping.pov);
            set(&mut next.behaviours, &key, (mapping.mode != Mode::Hold).then_some(mapping.mode));
        }
        Slot::Long | Slot::Double => {
            let targets = if slot == Slot::Long { &mut next.long_press } else { &mut next.double_tap };
            let target = (!mapping.buttons.is_empty() || mapping.pov.is_some()).then(|| Target { buttons: mapping.buttons.clone(), pov: mapping.pov });
            set(targets, &key, target);
        }
    }
    next
}

fn set<T>(map: &mut std::collections::BTreeMap<String, T>, key: &str, value: Option<T>) {
    match value {
        Some(value) => map.insert(key.to_string(), value),
        None => map.remove(key),
    };
}

/// The NEO axes and what drives them: (axis, source, game action).
pub fn axes(config: &Config) -> [(&'static str, String, &'static str); 4] {
    use neo_common::config::{HatMode, Lever, YawSource};
    let lever = if config.throttle_lever == Lever::Right { "Right" } else { "Left" };
    let yaw = match config.yaw_source {
        YawSource::Twist => "Sidestick twist",
        YawSource::Off => "No source (yaw off)",
        YawSource::LeverDifferential => "Quadrant lever difference",
    };
    let throttle = match config.yaw_source {
        YawSource::LeverDifferential => "Average of both quadrant levers or sidestick slider".to_string(),
        _ => format!("{lever} quadrant lever or sidestick slider"),
    };
    let hat = if config.hat == HatMode::Mapped { "No source (hat is mapped)" } else { "Sidestick hat" };
    let camera = format!("{hat}{}", if config.camera.lever { " + spare lever" } else { "" });
    [("X / Y", "Sidestick stick".into(), "Roll / pitch"), ("Rz", yaw.into(), "Yaw"), ("Z", throttle, "Throttle"), ("Rx / Ry", camera, "Camera")]
}
