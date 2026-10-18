//! Controller identities and layouts. A device is known by a stable id: a model id for the Thrustmaster
//! devices listed below, otherwise "<vid>-<pid>" in lowercase hex; a second identical device gets "#2".
use serde::Serialize;

pub const THRUSTMASTER: u16 = 0x044f;
/// The virtual NEO this bridge creates; never an input, never hidden.
pub const NEO_PID: u16 = 0x2800;

/// (product id, model id, display name). Only ids confirmed on real hardware or in public device lists; any other
/// Thrustmaster joystick works through the generic path under its "<vid>-<pid>" id and its own product string.
pub const MODELS: [(u16, &str, &str); 9] = [
    (0x0405, "tca-stick", "TCA Sidestick Airbus"),
    (0x0406, "tca-stick", "TCA Sidestick Airbus"),
    (0x0407, "tca-quadrant", "TCA Quadrant Eng 1&2"),
    (0x0402, "warthog-stick", "HOTAS Warthog joystick"),
    (0x0404, "warthog-throttle", "HOTAS Warthog throttle"),
    (0xb10a, "t16000m", "T.16000M"),
    (0xb687, "twcs", "TWCS Throttle"),
    (0xb108, "tflight-hotas-x", "T.Flight Hotas X"),
    (0xb67b, "tflight-hotas-4", "T.Flight Hotas 4"),
];

pub const TCA_STICK: &str = "tca-stick";
pub const TCA_QUADRANT: &str = "tca-quadrant";

pub fn model(vid: u16, pid: u16) -> Option<(&'static str, &'static str)> {
    MODELS.iter().find(|m| vid == THRUSTMASTER && m.0 == pid).map(|m| (m.1, m.2))
}

/// Model id, or "<vid>-<pid>" for devices without one.
pub fn base_id(vid: u16, pid: u16) -> String {
    model(vid, pid).map_or_else(|| format!("{vid:04x}-{pid:04x}"), |(id, _)| id.to_string())
}

/// Device id without the "#n" instance suffix.
pub fn strip(id: &str) -> &str {
    id.split('#').next().unwrap()
}

/// 1–32 of [a-z0-9-], optionally "#2" to "#99" for a further identical device.
pub fn valid_id(id: &str) -> bool {
    let (base, instance) = id.split_once('#').map_or((id, None), |(b, n)| (b, Some(n)));
    let base_ok = (1..=32).contains(&base.len()) && base.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    let instance_ok = instance.is_none_or(|n| n.parse::<u8>().is_ok_and(|n| (2..=99).contains(&n)) && !n.starts_with('0'));
    base_ok && instance_ok
}

/// Devices taken by default: every Thrustmaster joystick except the virtual NEO.
pub fn auto(vid: u16, pid: u16) -> bool {
    vid == THRUSTMASTER && pid != NEO_PID
}

/// Whether a device is part of the session: listed in `wanted` (by id, any instance), or `auto` when the list is empty.
pub fn wanted(wanted: &[String], vid: u16, pid: u16) -> bool {
    if vid == THRUSTMASTER && pid == NEO_PID {
        return false;
    }
    if wanted.is_empty() {
        return auto(vid, pid);
    }
    let base = base_id(vid, pid);
    wanted.iter().any(|w| strip(w) == base)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Axis {
    pub name: String,
    /// Raw values run from 0 to `max` after removing the logical minimum.
    pub max: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Layout {
    pub buttons: u16,
    pub axes: Vec<Axis>,
    pub hat: bool,
}

impl Layout {
    pub fn axis(&self, name: &str) -> Option<usize> {
        self.axes.iter().position(|a| a.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub layout: Layout,
}

fn axes(list: &[(&str, u32)]) -> Vec<Axis> {
    list.iter().map(|(name, max)| Axis { name: name.to_string(), max: *max }).collect()
}

/// TCA sidestick report 01: roll and pitch 14-bit, twist and slider 8-bit, hat, 17 inputs.
pub fn tca_stick() -> DeviceInfo {
    let layout = Layout { buttons: 17, axes: axes(&[("x", 16383), ("y", 16383), ("rz", 255), ("slider", 255)]), hat: true };
    DeviceInfo { id: TCA_STICK.into(), name: "TCA Sidestick Airbus".into(), vid: THRUSTMASTER, pid: 0x0405, layout }
}

/// TCA quadrant report 01: two 16-bit levers (forward is the low raw value), 31 inputs.
pub fn tca_quadrant() -> DeviceInfo {
    let layout = Layout { buttons: 31, axes: axes(&[("left", 65535), ("right", 65535)]), hat: false };
    DeviceInfo { id: TCA_QUADRANT.into(), name: "TCA Quadrant Eng 1&2".into(), vid: THRUSTMASTER, pid: 0x0407, layout }
}

/// Sorted device ids joined by "+": the key a mapping profile is stored under.
pub fn setup_key(devices: &[DeviceInfo]) -> String {
    let mut ids: Vec<&str> = devices.iter().map(|d| d.id.as_str()).collect();
    ids.sort_unstable();
    ids.join("+")
}

/// Give further identical devices "#2", "#3"… in enumeration order.
pub fn number(devices: &mut [DeviceInfo]) {
    for index in 0..devices.len() {
        let count = devices[..index].iter().filter(|d| strip(&d.id) == devices[index].id).count();
        if count > 0 {
            devices[index].id = format!("{}#{}", devices[index].id, count + 1);
        }
    }
}
