//! What the window edits: the settings shared by every controller set together with the mapping of the set in use,
//! as one flat value (the page's `flatten`/`compose`). The other sets' mappings ride along untouched, so every save
//! and preview writes the whole profile file back.
use neo_common::config::{
    Binding, Bindings, Camera, Combos, Config, Direction, HatMode, HeadTracking, Layer, Mode, Precision, Profile, SplitHighG, Target, Throttle,
    Timing, Tuning, YawSource, VERSION,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    // Shared by every controller set.
    pub axes: Tuning,
    pub camera_strength: f64,
    pub throttle: Throttle,
    pub safe_start: bool,
    pub timing: Timing,
    pub camera: Camera,
    pub head_tracking: HeadTracking,
    /// Device ids taking part; empty: every Thrustmaster joystick present.
    pub devices: Vec<String>,
    // The mapping of the set in use.
    pub hat: HatMode,
    pub camera_hat: Option<String>,
    pub bindings: Bindings,
    pub split_high_g: SplitHighG,
    pub camera_lever: Option<Binding>,
    pub yaw_source: YawSource,
    pub precision: Precision,
    pub combos: Combos,
    pub behaviours: BTreeMap<String, Mode>,
    pub long_press: BTreeMap<String, Target>,
    pub double_tap: BTreeMap<String, Target>,
    pub shift: Option<String>,
    pub shifted: Layer,
    pub buttons: BTreeMap<String, Vec<u8>>,
    pub pov: BTreeMap<String, Direction>,
    /// The set in use ("tca-quadrant+tca-stick"), and every set's saved mapping as the file holds them.
    pub setup: String,
    pub setups: BTreeMap<String, Profile>,
}

impl View {
    /// The file seen from `setup`: its saved mapping, or `default` while the set has none.
    pub fn of(file: &Config, setup: &str, default: &Profile) -> Self {
        let profile = file.setups.get(setup).unwrap_or(default).clone();
        Self {
            axes: file.axes,
            camera_strength: file.camera_strength,
            throttle: file.throttle,
            safe_start: file.safe_start,
            timing: file.timing,
            camera: file.camera,
            head_tracking: file.head_tracking,
            devices: file.devices.clone(),
            hat: profile.hat,
            camera_hat: profile.camera_hat,
            bindings: profile.bindings,
            split_high_g: profile.split_high_g,
            camera_lever: profile.camera_lever,
            yaw_source: profile.yaw_source,
            precision: profile.precision,
            combos: profile.combos,
            behaviours: profile.behaviours,
            long_press: profile.long_press,
            double_tap: profile.double_tap,
            shift: profile.shift,
            shifted: profile.shifted,
            buttons: profile.buttons,
            pov: profile.pov,
            setup: setup.to_string(),
            setups: file.setups.clone(),
        }
    }

    /// This set's mapping.
    pub fn profile(&self) -> Profile {
        Profile {
            hat: self.hat,
            camera_hat: self.camera_hat.clone(),
            bindings: self.bindings.clone(),
            split_high_g: self.split_high_g.clone(),
            camera_lever: self.camera_lever.clone(),
            yaw_source: self.yaw_source,
            precision: self.precision.clone(),
            combos: self.combos.clone(),
            behaviours: self.behaviours.clone(),
            long_press: self.long_press.clone(),
            double_tap: self.double_tap.clone(),
            shift: self.shift.clone(),
            shifted: self.shifted.clone(),
            buttons: self.buttons.clone(),
            pov: self.pov.clone(),
        }
    }

    /// The whole profile file, with this set's mapping stored under its key.
    pub fn compose(&self) -> Config {
        let mut setups = self.setups.clone();
        setups.insert(self.setup.clone(), self.profile());
        Config {
            version: VERSION,
            axes: self.axes,
            camera_strength: self.camera_strength,
            throttle: self.throttle,
            safe_start: self.safe_start,
            timing: self.timing,
            camera: self.camera,
            head_tracking: self.head_tracking,
            devices: self.devices.clone(),
            setups,
        }
    }
}
