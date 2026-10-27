//! The settings pages' draft: every setting is edited here first, previewed live, optionally run in game ("Try in
//! game"), then saved or discarded. Number fields keep their text while it is being typed, so a half-typed value
//! is never written into the draft.
use crate::mapping;
use crate::previews::Press;
use crate::worker::Request;
use neo_common::config::{AxisTuning, Config, YawSource};
use std::collections::BTreeMap;

/// The flight axis tuned on the Flight axes page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis {
    Roll,
    Pitch,
    Yaw,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::Roll, Axis::Pitch, Axis::Yaw];

    pub fn name(self) -> &'static str {
        match self {
            Axis::Roll => "Roll",
            Axis::Pitch => "Pitch",
            Axis::Yaw => "Yaw",
        }
    }

    pub fn tuning(self, config: &Config) -> &AxisTuning {
        match self {
            Axis::Roll => &config.axes.roll,
            Axis::Pitch => &config.axes.pitch,
            Axis::Yaw => &config.axes.yaw,
        }
    }

    pub fn tuning_mut(self, config: &mut Config) -> &mut AxisTuning {
        match self {
            Axis::Roll => &mut config.axes.roll,
            Axis::Pitch => &mut config.axes.pitch,
            Axis::Yaw => &mut config.axes.yaw,
        }
    }

    /// The axes "Copy" copies this one's settings to.
    pub fn copy_targets(self) -> &'static [Axis] {
        match self {
            Axis::Roll => &[Axis::Pitch],
            Axis::Pitch => &[Axis::Roll],
            Axis::Yaw => &[Axis::Roll, Axis::Pitch],
        }
    }
}

/// Every number field of the settings pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    Deadzone,
    Saturation,
    AntiDeadzone,
    Curve,
    Smoothing,
    CameraStrength,
    CameraRamp,
    LongPress,
    DoubleTap,
    Repeat,
    Pulse,
    Idle,
    Cruise,
    Boost,
    Band,
    PrecisionScale,
    SplitForward,
    SplitBack,
    HeadPort,
    HeadYaw,
    HeadPitch,
    HeadTimeout,
    HeadDeadzone,
    HeadCurve,
}

/// Fields that share one error line under a group of cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errors {
    Own,
    Timing,
    Detents,
    Head,
}

impl Field {
    pub const ALL: [Field; 24] = [
        Field::Deadzone, Field::Saturation, Field::AntiDeadzone, Field::Curve, Field::Smoothing, Field::CameraStrength, Field::CameraRamp,
        Field::LongPress, Field::DoubleTap, Field::Repeat, Field::Pulse, Field::Idle, Field::Cruise, Field::Boost, Field::Band,
        Field::PrecisionScale, Field::SplitForward, Field::SplitBack, Field::HeadPort, Field::HeadYaw, Field::HeadPitch, Field::HeadTimeout,
        Field::HeadDeadzone, Field::HeadCurve,
    ];

    /// (minimum, maximum, step) as shown, the factor from the profile's value (100 for percentages), and the error
    /// line it reports on.
    pub fn spec(self) -> (f64, f64, f64, f64, Errors) {
        use Errors::*;
        match self {
            Field::Deadzone | Field::AntiDeadzone => (0.0, 50.0, 1.0, 100.0, Own),
            Field::Saturation => (50.0, 100.0, 1.0, 100.0, Own),
            Field::Curve => (-100.0, 100.0, 1.0, 100.0, Own),
            Field::Smoothing => (0.0, 90.0, 5.0, 100.0, Own),
            Field::CameraStrength => (0.0, 100.0, 5.0, 100.0, Own),
            Field::CameraRamp => (0.0, 2000.0, 50.0, 1.0, Own),
            Field::LongPress => (200.0, 2000.0, 50.0, 1.0, Timing),
            Field::DoubleTap => (100.0, 1000.0, 10.0, 1.0, Timing),
            Field::Repeat => (30.0, 1000.0, 10.0, 1.0, Timing),
            Field::Pulse => (20.0, 1000.0, 10.0, 1.0, Timing),
            Field::Idle | Field::Cruise | Field::Boost => (0.0, 100.0, 1.0, 100.0, Detents),
            Field::Band => (0.0, 20.0, 1.0, 100.0, Detents),
            Field::PrecisionScale => (10.0, 100.0, 5.0, 100.0, Own),
            Field::SplitForward | Field::SplitBack => (0.0, 100.0, 1.0, 100.0, Own),
            Field::HeadPort => (1024.0, 65535.0, 1.0, 1.0, Head),
            Field::HeadYaw => (5.0, 180.0, 1.0, 1.0, Head),
            Field::HeadPitch => (5.0, 90.0, 1.0, 1.0, Head),
            Field::HeadTimeout => (100.0, 5000.0, 50.0, 1.0, Head),
            Field::HeadDeadzone => (0.0, 50.0, 1.0, 100.0, Head),
            Field::HeadCurve => (-100.0, 100.0, 1.0, 100.0, Head),
        }
    }

    /// The field's value in the profile (the axis fields read the tuned axis).
    pub fn get(self, config: &Config, axis: Axis) -> f64 {
        let tuning = axis.tuning(config);
        let head = &config.head_tracking;
        match self {
            Field::Deadzone => tuning.deadzone,
            Field::Saturation => tuning.saturation,
            Field::AntiDeadzone => tuning.anti_deadzone,
            Field::Curve => tuning.curve,
            Field::Smoothing => tuning.smoothing,
            Field::CameraStrength => config.camera_strength,
            Field::CameraRamp => f64::from(config.camera.ramp_ms),
            Field::LongPress => f64::from(config.timing.long_ms),
            Field::DoubleTap => f64::from(config.timing.double_ms),
            Field::Repeat => f64::from(config.timing.repeat_ms),
            Field::Pulse => f64::from(config.timing.pulse_ms),
            Field::Idle => config.throttle.idle,
            Field::Cruise => config.throttle.cruise,
            Field::Boost => config.throttle.boost,
            Field::Band => config.throttle.band,
            Field::PrecisionScale => config.precision.scale,
            Field::SplitForward => config.split_high_g.forward,
            Field::SplitBack => config.split_high_g.back,
            Field::HeadPort => f64::from(head.port),
            Field::HeadYaw => head.yaw_range,
            Field::HeadPitch => head.pitch_range,
            Field::HeadTimeout => f64::from(head.timeout_ms),
            Field::HeadDeadzone => head.tuning.deadzone,
            Field::HeadCurve => head.tuning.curve,
        }
    }

    /// Write the field's value into the profile (whole numbers for the integer settings).
    pub fn set(self, config: &mut Config, axis: Axis, value: f64) {
        let whole = |value: f64| value.round().clamp(0.0, f64::from(u32::MAX)) as u32;
        match self {
            Field::Deadzone => axis.tuning_mut(config).deadzone = value,
            Field::Saturation => axis.tuning_mut(config).saturation = value,
            Field::AntiDeadzone => axis.tuning_mut(config).anti_deadzone = value,
            Field::Curve => axis.tuning_mut(config).curve = value,
            Field::Smoothing => axis.tuning_mut(config).smoothing = value,
            Field::CameraStrength => config.camera_strength = value,
            Field::CameraRamp => config.camera.ramp_ms = whole(value),
            Field::LongPress => config.timing.long_ms = whole(value),
            Field::DoubleTap => config.timing.double_ms = whole(value),
            Field::Repeat => config.timing.repeat_ms = whole(value),
            Field::Pulse => config.timing.pulse_ms = whole(value),
            Field::Idle => config.throttle.idle = value,
            Field::Cruise => config.throttle.cruise = value,
            Field::Boost => config.throttle.boost = value,
            Field::Band => config.throttle.band = value,
            Field::PrecisionScale => config.precision.scale = value,
            Field::SplitForward => config.split_high_g.forward = value,
            Field::SplitBack => config.split_high_g.back = value,
            Field::HeadPort => config.head_tracking.port = value.round().clamp(0.0, 65535.0) as u16,
            Field::HeadYaw => config.head_tracking.yaw_range = value,
            Field::HeadPitch => config.head_tracking.pitch_range = value,
            Field::HeadTimeout => config.head_tracking.timeout_ms = whole(value),
            Field::HeadDeadzone => config.head_tracking.tuning.deadzone = value,
            Field::HeadCurve => config.head_tracking.tuning.curve = value,
        }
    }
}

/// The save bar's message: saved (green), unsaved (amber) or a problem (red).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Saved,
    Dirty,
    Error,
}

/// A control picked by pressing it: the precision or shift control, or a combo being recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Precision,
    Shift,
    Pause,
    Next,
}

impl Kind {
    /// What it is called in messages.
    pub fn text(self) -> &'static str {
        match self {
            Kind::Precision => "precision aiming",
            Kind::Shift => "the shift layer",
            Kind::Pause => "pause",
            Kind::Next => "next-profile",
        }
    }

    pub fn combo(self, config: &Config) -> &Vec<String> {
        if self == Kind::Pause {
            &config.combos.pause
        } else {
            &config.combos.next_profile
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub kind: Kind,
    pub keys: Vec<String>,
}

/// The press timeline spans this many milliseconds.
pub const PRESS_SPAN: f64 = 3000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The draft (None until the profile loads).
    pub draft: Option<Config>,
    /// The text of each number field.
    pub fields: BTreeMap<Field, String>,
    /// Problems by field, after a save or preview was attempted.
    pub errors: BTreeMap<Field, String>,
    pub axis: Axis,
    pub dirty: bool,
    pub message: Option<(String, Tone)>,
    /// The test controls, used by the previews only without live input.
    pub test_input: f64,
    pub test_lever: f64,
    /// The draft runs in game instead of the saved profile.
    pub previewing: bool,
    /// A preview request is in flight, and another is waiting behind it.
    pub sending: bool,
    pub queued: bool,
    /// A save is in flight.
    pub saving: bool,
    pub capture: Option<Capture>,
    /// Recent presses for the timing timeline: (source, press).
    pub presses: Vec<(String, Press)>,
    /// When the hat was pressed, while it is held (and drives the camera).
    pub hat_since: Option<f64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            draft: None,
            fields: BTreeMap::new(),
            errors: BTreeMap::new(),
            axis: Axis::Roll,
            dirty: false,
            message: None,
            test_input: 0.4,
            test_lever: 0.5,
            previewing: false,
            sending: false,
            queued: false,
            saving: false,
            capture: None,
            presses: Vec::new(),
            hat_since: None,
        }
    }
}

/// The quadrant levers' three users: only one may be on at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeverUser {
    Split,
    Camera,
    Yaw,
}

impl Settings {
    /// Start from a profile: the draft, its field texts and no capture.
    pub fn show(&mut self, config: Config) {
        self.draft = Some(config);
        self.capture = None;
        self.fill();
    }

    /// The field texts from the draft (the axis fields show the tuned axis); clears the problems.
    pub fn fill(&mut self) {
        let Some(config) = &self.draft else { return };
        self.fields = Field::ALL.iter().map(|field| (*field, format!("{}", (field.get(config, self.axis) * field.spec().3).round()))).collect();
        self.errors.clear();
    }

    pub fn set_axis(&mut self, axis: Axis) {
        self.axis = axis;
        self.fill();
    }

    fn mark(&mut self, dirty: bool, text: &str) {
        self.dirty = dirty;
        self.message = Some((text.into(), if dirty { Tone::Dirty } else { Tone::Saved }));
    }

    fn problem(&mut self, text: String) {
        self.message = Some((text, Tone::Error));
    }

    /// The draft changed: say so, and send it to the game while trying it there.
    pub fn changed(&mut self, text: &str) -> Vec<Request> {
        let text = if self.previewing { "Unsaved changes \u{b7} running in game now." } else { text };
        self.mark(true, text);
        self.push_preview()
    }

    /// Edit the draft with `edit`, then mark it changed.
    pub fn edit(&mut self, text: &str, edit: impl FnOnce(&mut Config)) -> Vec<Request> {
        self.quietly(edit);
        self.changed(text)
    }

    /// A number field (or its slider) changed: the text is kept, and a number is taken into the draft.
    pub fn type_number(&mut self, field: Field, text: &str) -> Vec<Request> {
        self.fields.insert(field, text.to_string());
        let axis = self.axis;
        match text.trim().parse::<f64>().ok().filter(|value| value.is_finite()) {
            Some(value) => self.edit("Unsaved changes.", |config| field.set(config, axis, value / field.spec().3)),
            None => self.changed("Unsaved changes."),
        }
    }

    /// Turn on one of the levers' users, turning the others off and saying so.
    pub fn exclusive(&mut self, chosen: LeverUser) -> Vec<Request> {
        let Some(config) = self.draft.as_mut() else { return Vec::new() };
        let users = [
            (LeverUser::Split, "split-lever High-G", config.split_high_g.enabled),
            (LeverUser::Camera, "the camera lever", config.camera.lever),
            (LeverUser::Yaw, "lever yaw", config.yaw_source == YawSource::LeverDifferential),
        ];
        let off: Vec<&str> = users.iter().filter(|(user, _, on)| *user != chosen && *on).map(|(_, text, _)| *text).collect();
        for (user, _, _) in users.iter().filter(|(user, _, on)| *user != chosen && *on) {
            match user {
                LeverUser::Split => config.split_high_g.enabled = false,
                LeverUser::Camera => config.camera.lever = false,
                LeverUser::Yaw => config.yaw_source = YawSource::Twist,
            }
        }
        let text = if off.is_empty() { "Unsaved changes.".to_string() } else { format!("Unsaved changes: {} turned off, since it uses the quadrant levers too.", off.join(" and ")) };
        self.changed(&text)
    }

    /// Change the draft without marking it (the caller says what changed).
    fn quietly(&mut self, edit: impl FnOnce(&mut Config)) {
        self.change(Box::new(edit));
    }

    /// One copy of the branch for every edit (a generic one is counted per closure).
    fn change(&mut self, edit: Box<dyn FnOnce(&mut Config) + '_>) {
        if let Some(config) = self.draft.as_mut() {
            edit(config);
        }
    }

    /// A lever user turned on takes the levers from the others.
    fn lever_user(&mut self, on: bool, user: LeverUser) -> Vec<Request> {
        if on {
            self.exclusive(user)
        } else {
            self.changed("Unsaved changes.")
        }
    }

    pub fn set_split(&mut self, on: bool) -> Vec<Request> {
        self.quietly(|config| config.split_high_g.enabled = on);
        self.lever_user(on, LeverUser::Split)
    }

    pub fn set_camera_lever(&mut self, on: bool) -> Vec<Request> {
        self.quietly(|config| config.camera.lever = on);
        self.lever_user(on, LeverUser::Camera)
    }

    pub fn set_yaw_source(&mut self, source: YawSource) -> Vec<Request> {
        self.quietly(|config| config.yaw_source = source);
        self.lever_user(source == YawSource::LeverDifferential, LeverUser::Yaw)
    }
    /// Copy the tuned axis to the others ("Copy roll to pitch").
    pub fn copy_axis(&mut self) -> Vec<Request> {
        let axis = self.axis;
        let targets = axis.copy_targets();
        let names: Vec<String> = targets.iter().map(|target| target.name().to_lowercase()).collect();
        let text = format!("Unsaved: {} settings copied to {}.", axis.name(), names.join(" and "));
        self.edit(&text, |config| {
            let tuning = *axis.tuning(config);
            for target in targets {
                *target.tuning_mut(config) = tuning;
            }
        })
    }

    /// The tuned axis back to its defaults.
    pub fn reset_axis(&mut self, defaults: &Config) -> Vec<Request> {
        let axis = self.axis;
        let tuning = *axis.tuning(defaults);
        let requests = self.edit(&format!("Unsaved: {} back to its defaults.", axis.name()), |config| *axis.tuning_mut(config) = tuning);
        self.fill();
        requests
    }

    /// Every setting, every mapping included, back to the defaults (unsaved).
    pub fn reset(&mut self, defaults: &Config) -> Vec<Request> {
        self.show(defaults.clone());
        self.changed("Unsaved: defaults, including every mapping. Save settings to apply.")
    }

    /// A throttle detent set to where the lever is now.
    pub fn capture_detent(&mut self, field: Field, travel: f64) -> Vec<Request> {
        let travel = (travel * 100.0).round() / 100.0;
        let name = match field {
            Field::Cruise => "CL (cruise)",
            Field::Idle => "Idle",
            _ => "Boost",
        };
        let axis = self.axis;
        let requests = self.edit(&format!("Unsaved: {name} captured at {}%.", (travel * 100.0).round()), |config| field.set(config, axis, travel));
        self.fields.insert(field, format!("{}", (travel * 100.0).round()));
        requests
    }

    /// Check every field and the rules between them; on success the draft takes exactly what the fields show.
    pub fn validate(&mut self) -> bool {
        self.errors.clear();
        let value = |fields: &BTreeMap<Field, String>, field: Field| fields.get(&field).and_then(|text| text.trim().parse::<f64>().ok()).filter(|value| value.is_finite());
        for field in Field::ALL {
            let (low, high, ..) = field.spec();
            if !value(&self.fields, field).is_some_and(|value| (low..=high).contains(&value)) {
                self.errors.insert(field, format!("Enter a number from {low} to {high}."));
            }
        }
        if self.errors.is_empty() {
            let v = |field| value(&self.fields, field).unwrap_or_default();
            let checks = [
                (Field::Saturation, v(Field::Saturation) <= v(Field::Deadzone), "Must be beyond the deadzone."),
                (Field::Repeat, v(Field::Repeat) <= v(Field::Pulse), "Repeat every must be longer than the pulse length."),
                (Field::Cruise, v(Field::Idle) >= v(Field::Cruise) - v(Field::Band) || v(Field::Cruise) + v(Field::Band) >= v(Field::Boost), "Needs idle < cruise \u{2212} band and cruise + band < boost."),
                (Field::SplitBack, v(Field::SplitBack) >= v(Field::SplitForward), "Must be below the forward lever threshold."),
            ];
            for (field, wrong, text) in checks {
                if wrong {
                    self.errors.insert(field, text.into());
                }
            }
        }
        if !self.errors.is_empty() {
            self.problem("Fix the highlighted fields, then save.".into());
            return false;
        }
        let axis = self.axis;
        let values: Vec<(Field, f64)> = Field::ALL.iter().map(|field| (*field, value(&self.fields, *field).unwrap_or_default() / field.spec().3)).collect();
        if let Some(config) = self.draft.as_mut() {
            for (field, value) in values {
                field.set(config, axis, value);
            }
        }
        true
    }

    /// The problem shown for a field, or for the group of cards it belongs to.
    pub fn error(&self, field: Field) -> Option<&String> {
        let group = field.spec().4;
        if group == Errors::Own {
            return self.errors.get(&field);
        }
        Field::ALL.iter().filter(|other| other.spec().4 == group).find_map(|other| self.errors.get(other))
    }

    /// Save the draft (a save also ends any preview).
    pub fn save(&mut self) -> Vec<Request> {
        if self.saving || !self.validate() {
            return Vec::new();
        }
        self.saving = true;
        self.message = Some(("Saving\u{2026}".into(), Tone::Dirty));
        self.draft.clone().map(|config| Request::Save(Box::new(config))).into_iter().collect()
    }

    pub fn saved(&mut self, running: bool) {
        self.saving = false;
        self.previewing = false;
        self.mark(false, if running { "Saved. Applied to the running emulation." } else { "Saved." });
    }

    pub fn save_failed(&mut self, error: &str) {
        self.saving = false;
        self.problem(format!("Not saved: {error}"));
    }

    /// Back to the saved profile, ending any preview.
    pub fn discard(&mut self, saved: &Config) -> Vec<Request> {
        let requests = self.set_preview(false);
        self.show(saved.clone());
        self.mark(false, "Changes discarded.");
        requests
    }

    /// "Try in game": start or end running the draft in the emulation.
    pub fn set_preview(&mut self, on: bool) -> Vec<Request> {
        match on {
            true if !self.validate() => Vec::new(),
            true => {
                self.previewing = true;
                self.push_preview()
            }
            false if self.previewing => {
                self.previewing = false;
                vec![Request::EndPreview]
            }
            false => Vec::new(),
        }
    }

    /// Send the draft while previewing; drags coalesce into one request in flight plus the latest.
    pub fn push_preview(&mut self) -> Vec<Request> {
        if !self.previewing || !self.validate() {
            return Vec::new();
        }
        if self.sending {
            self.queued = true;
            return Vec::new();
        }
        self.sending = true;
        self.draft.clone().map(|config| Request::Preview(Box::new(config))).into_iter().collect()
    }

    pub fn previewed(&mut self) -> Vec<Request> {
        self.sending = false;
        if self.previewing {
            self.mark(true, "Unsaved changes \u{b7} running in game now.");
        }
        if std::mem::take(&mut self.queued) {
            return self.push_preview();
        }
        Vec::new()
    }

    pub fn preview_failed(&mut self, error: &str) {
        self.sending = false;
        self.queued = false;
        self.previewing = false;
        self.problem(format!("Preview stopped: {error}"));
    }

    pub fn preview_end_failed(&mut self, error: &str) {
        self.problem(format!("Preview not ended: {error}"));
    }

    /// The window opened on a preview already running: the draft is that preview.
    pub fn resume_preview(&mut self) {
        self.previewing = true;
        self.mark(true, "Unsaved changes \u{b7} running in game now.");
    }

    /// The daemon ended the preview itself (emulation stopped).
    pub fn preview_gone(&mut self) {
        if self.previewing && !self.sending {
            self.previewing = false;
            self.mark(true, "Unsaved changes \u{b7} the preview ended with emulation.");
        }
    }

    /// Leaving the settings pages ends a preview, keeping the draft.
    pub fn leave(&mut self) -> Vec<Request> {
        let requests = self.set_preview(false);
        if !requests.is_empty() {
            self.mark(true, "Unsaved changes \u{b7} preview ended when you left the settings.");
        }
        requests
    }

    /// The precision or shift control of the draft.
    pub fn single(&self, kind: Kind) -> Option<&String> {
        let config = self.draft.as_ref()?;
        if kind == Kind::Precision {
            config.precision.source.as_ref()
        } else {
            config.shift.as_ref()
        }
    }

    fn set_single(config: &mut Config, kind: Kind, key: Option<String>) {
        if kind == Kind::Precision {
            config.precision.source = key;
        } else {
            config.shift = key;
        }
    }

    pub fn clear_single(&mut self, kind: Kind) -> Vec<Request> {
        let text = format!("Unsaved: {} off.", if kind == Kind::Precision { "precision aiming" } else { "shift layer" });
        self.edit(&text, |config| Self::set_single(config, kind, None))
    }

    /// Start picking a control by pressing it, or cancel the pick.
    pub fn start_capture(&mut self, kind: Kind) {
        self.capture = match &self.capture {
            Some(capture) if capture.kind == kind => None,
            _ => Some(Capture { kind, keys: Vec::new() }),
        };
    }

    /// Record combo… then Done: the pressed controls become the combo (none turns it off).
    pub fn record_combo(&mut self, kind: Kind) -> Vec<Request> {
        let Some(capture) = self.capture.take().filter(|capture| capture.kind == kind) else {
            self.start_capture(kind);
            return Vec::new();
        };
        let text = if capture.keys.is_empty() { format!("Unsaved: {} combo off.", kind.text()) } else { format!("Unsaved: new {} combo.", kind.text()) };
        self.edit(&text, |config| {
            if kind == Kind::Pause {
                config.combos.pause = capture.keys;
            } else {
                config.combos.next_profile = capture.keys;
            }
        })
    }

    pub fn clear_combo(&mut self, kind: Kind) -> Vec<Request> {
        self.edit(&format!("Unsaved: {} combo off.", kind.text()), |config| {
            if kind == Kind::Pause {
                config.combos.pause.clear();
            } else {
                config.combos.next_profile.clear();
            }
        })
    }

    /// Live input: the press history for the timing timeline, the hat hold for the camera ramp, and a capture in
    /// progress (`fresh`: buttons newly pressed). `now` is the window's clock in milliseconds.
    pub fn press(&mut self, held: &[String], fresh: &[String], hat_drives_camera: bool, now: f64) -> Vec<Request> {
        for (key, press) in self.presses.iter_mut() {
            if press.end.is_none() && !held.contains(key) {
                press.end = Some(now);
            }
        }
        self.presses.extend(fresh.iter().map(|key| (key.clone(), Press { start: now, end: None })));
        self.presses.retain(|(_, press)| press.end.is_none_or(|end| end >= now - PRESS_SPAN));
        let hat = hat_drives_camera && held.iter().any(|key| key.starts_with("hat-"));
        self.hat_since = if hat { self.hat_since.or(Some(now)) } else { None };
        let buttons: Vec<&String> = fresh.iter().filter(|key| !key.starts_with("hat-")).collect();
        match self.capture.as_mut().map(|capture| capture.kind) {
            Some(kind @ (Kind::Precision | Kind::Shift)) if !buttons.is_empty() => {
                let key = buttons[0].clone();
                self.capture = None;
                let text = format!("Unsaved: hold {} for {}.", mapping::label(&key), kind.text());
                self.edit(&text, |config| Self::set_single(config, kind, Some(key)))
            }
            Some(Kind::Pause | Kind::Next) => {
                let capture = self.capture.as_mut().expect("capturing");
                for key in buttons {
                    if !capture.keys.contains(key) && capture.keys.len() < 4 {
                        capture.keys.push(key.clone());
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// The presses of the latest control pressed, and that control.
    pub fn latest(&self) -> Option<(&String, Vec<crate::previews::Press>)> {
        let (key, _) = self.presses.last()?;
        Some((key, self.presses.iter().filter(|(other, _)| other == key).map(|(_, press)| *press).collect()))
    }
}
