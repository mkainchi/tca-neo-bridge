//! The settings pages: Flight axes, Throttle, Camera & head, and Buttons & helpers. Every control edits the draft,
//! and every preview draws the draft against live input (the test controls stand in without it).
use super::widgets::{self, bar, card, cap, help, live_chip, mono, number, panel, panel_head, seg, slider, state as chip_state, toggle, Kind as Button};
use crate::mapping;
use crate::previews::{self, axis_value, degrees, gestures, head_camera, signed, throttle_out, zone, Gesture, Tone, RAMP_SPAN_MS};
use crate::settings::{Axis, Field, Kind, Settings, PRESS_SPAN};
use crate::state::{Page, State};
use crate::theme::{self, tint};
use crate::worker::Request;
use eframe::egui::{self, epaint::PathShape, Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2};
use neo_common::api::Monitor;
use neo_common::config::{Config, HatMode, Lever, ThrottleMode, YawSource};
use neo_common::maths::shape;

/// The amber of the deadzone band, the brake hatch and the long-press marker.
const ADDED: Color32 = Color32::from_rgb(0xfa, 0xcc, 0x15);
const RAW: Color32 = Color32::from_rgba_premultiplied(89, 98, 110, 153);

/// Split `rect` into columns in proportion to `weights`, `gap` apart.
pub fn columns(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect> {
    let total: f32 = weights.iter().sum();
    let width = rect.width() - gap * (weights.len() as f32 - 1.0);
    let mut left = rect.left();
    weights
        .iter()
        .map(|weight| {
            let column = Rect::from_min_size(Pos2::new(left, rect.top()), Vec2::new(width * weight / total, rect.height()));
            left = column.right() + gap;
            column
        })
        .collect()
}

/// What the previews read: the device while emulating from the TCA, else the test controls.
pub struct Inputs<'a> {
    pub monitor: Option<&'a Monitor>,
    pub levers: Option<[f64; 2]>,
    pub held: &'a [String],
    pub emulating: bool,
    pub saved_hat: HatMode,
    pub now: f64,
}

/// The page body under its head; returns the requests to send.
pub fn page(ui: &mut Ui, state: &mut State, compact: bool) -> Vec<Request> {
    let mut requests = Vec::new();
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let defaults = state.defaults.clone().expect("the defaults arrive with the profile");
    let inputs = Inputs {
        monitor: state.monitor.as_ref(),
        levers: state.levers,
        held: &state.map.held,
        emulating: state.map.emulating,
        saved_hat: state.saved.as_ref().map_or(HatMode::Camera, |saved| saved.hat),
        now: state.now,
    };
    let s = &mut state.settings;
    match state.page {
        Page::Axes => axes(ui, full, s, &inputs, &defaults, compact, &mut requests),
        Page::Throttle => throttle(ui, full, s, &inputs, &mut requests),
        Page::Camera => camera(ui, full, s, &inputs, &mut requests),
        _ => helpers(ui, full, s, &inputs, &mut requests),
    }
    requests
}

/// The page head's right-hand switch: the tuned axis, or the throttle response.
pub fn head_switch(ui: &mut Ui, state: &mut State) -> Vec<Request> {
    let s = &mut state.settings;
    match state.page {
        Page::Axes => {
            let options = Axis::ALL.map(|axis| (axis.name(), axis.name(), true));
            if let Some(index) = seg(ui, &options, Axis::ALL.iter().position(|axis| *axis == s.axis).expect("an axis"), None) {
                s.set_axis(Axis::ALL[index]);
            }
            Vec::new()
        }
        Page::Throttle => {
            let detents = s.draft.as_ref().is_some_and(|config| config.throttle.mode == ThrottleMode::Detents);
            match seg(ui, &[("Linear", "Linear throttle", true), ("Detents", "Detent throttle", true)], usize::from(detents), None) {
                Some(index) => s.edit("Unsaved changes.", |config| config.throttle.mode = [ThrottleMode::Linear, ThrottleMode::Detents][index]),
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// A label and its help text, stacked; returns the label's response (it labels the field beside it).
fn label(ui: &mut Ui, title: &str, help_text: Option<&str>) -> egui::Response {
    // Leave room for the number field and its unit on the right.
    let width = ui.available_width() - 96.0;
    ui.vertical(|ui| {
        ui.set_max_width(width);
        ui.spacing_mut().item_spacing.y = 1.0;
        let label = ui.label(RichText::new(title).size(12.5).color(theme::INK).strong());
        if let Some(text) = help_text {
            ui.label(help(text));
        }
        label
    })
    .inner
}

/// A number field with its unit; its problem shows in red.
fn number_unit(ui: &mut Ui, s: &mut Settings, field: Field, label: &egui::Response, unit: &str, width: f32, requests: &mut Vec<Request>) {
    let mut text = s.fields.get(&field).cloned().unwrap_or_default();
    // The unit always follows the number: added first in a right-to-left row.
    let unit = |ui: &mut Ui| {
        if !unit.is_empty() {
            ui.label(RichText::new(unit).font(mono(11.0)).color(theme::MUTED));
        }
    };
    let rtl = ui.layout().prefer_right_to_left();
    if rtl {
        unit(ui);
    }
    if number(ui, &mut text, label, width, s.errors.contains_key(&field)) {
        requests.extend(s.type_number(field, &text));
    }
    if !rtl {
        unit(ui);
    }
}

fn error(ui: &mut Ui, s: &Settings, field: Field) {
    if let Some(text) = s.error(field) {
        ui.label(RichText::new(text).font(mono(11.5)).color(theme::DANGER_INK));
    }
}

/// A setting: its label and help, its number field, a slider (amber for `warn`), and its problem.
#[allow(clippy::too_many_arguments)]
fn field(ui: &mut Ui, s: &mut Settings, field: Field, title: &str, help_text: Option<&str>, unit: &str, warn: bool, requests: &mut Vec<Request>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            let label = label(ui, title, help_text);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| number_unit(ui, s, field, &label, unit, 64.0, requests));
        });
        let (low, high, step, ..) = field.spec();
        let mut value = s.fields.get(&field).and_then(|text| text.trim().parse::<f64>().ok()).unwrap_or(low).clamp(low, high);
        if slider(ui, &mut value, (low, high), step, warn, &format!("{title}, {}", if unit == "%" { "percent" } else { "milliseconds" })) {
            requests.extend(s.type_number(field, &format!("{value}")));
        }
        error(ui, s, field);
    });
}

/// A card with a label on the left and its number on the right.
fn number_card(ui: &mut Ui, s: &mut Settings, field: Field, (title, unit): (&str, &str), width: f32, requests: &mut Vec<Request>) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            let room = ui.available_width() - width - 24.0;
            let label = ui.allocate_ui_with_layout(Vec2::new(room, 30.0), Layout::left_to_right(Align::Center), |ui| ui.add(egui::Label::new(RichText::new(title).size(12.0).color(theme::INK).strong()).truncate())).inner;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| number_unit(ui, s, field, &label, unit, width, requests));
        });
    });
}

/// A grid of equal cells across the available width.
fn grid<T>(ui: &mut Ui, items: Vec<T>, per_row: usize, height: f32, mut add: impl FnMut(&mut Ui, T)) {
    let width = ui.available_width();
    let gap = 8.0;
    let cell = (width - gap * (per_row as f32 - 1.0)) / per_row as f32;
    let mut items = items.into_iter().peekable();
    while items.peek().is_some() {
        let (row, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        for (index, item) in items.by_ref().take(per_row).enumerate() {
            let rect = Rect::from_min_size(Pos2::new(row.left() + index as f32 * (cell + gap), row.top()), Vec2::new(cell, height));
            // A child of its own: the cells must not move this row's cursor (a short last cell would pull it up).
            add(&mut ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min))), item);
        }
    }
}

/// A readout card: a caption over a large mono value.
fn read(ui: &mut Ui, title: &str, value: &str, out: bool) {
    card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.label(cap(title));
        ui.label(RichText::new(value).font(mono(15.0)).color(if out { theme::ACCENT } else { theme::WHITE }).strong());
    });
}

/// The dashed note shown with the test controls.
fn idle_note(ui: &mut Ui, text: &str) {
    let response = egui::Frame::new().inner_margin(egui::Margin::symmetric(9, 6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(text).size(11.5).color(theme::MUTED));
    });
    let rect = response.response.rect;
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    ui.painter().extend(Shape::dashed_line(&corners, Stroke::new(1.0, theme::LINE_2), 4.0, 3.0));
}

fn test_slider(ui: &mut Ui, value: &mut f64, range: (f64, f64), label: &str, note: &str) -> bool {
    idle_note(ui, note);
    let mut percent = (*value * 100.0).round();
    let changed = slider(ui, &mut percent, range, 1.0, false, label);
    *value = percent / 100.0;
    changed
}

/// A head line with an uppercase heading on the left and a live chip on the right.
fn live_head(ui: &mut Ui, title: &str, live: bool) {
    panel_head(ui, |ui| {
        ui.label(widgets::h3(title));
    }, |ui| live_chip(ui, live));
}

fn tone_head(ui: &mut Ui, title: &str, (text, tone): (String, Tone)) {
    panel_head(ui, |ui| {
        ui.label(widgets::h3(title));
    }, |ui| chip_state(ui, &text, tone));
}

// ---------------------------------------------------------------------------------------------------------------
// Flight axes

/// The response plot: -1..1 both ways in a square, with the deadzone band, the identity line and the input's dot.
fn curve_plot(ui: &mut Ui, room: Rect, config: &Config, axis: Axis, input: f64) {
    let side = room.width().min(room.height());
    let rect = Rect::from_center_size(room.center(), Vec2::splat(side));
    let tuning = axis.tuning(config);
    let response = ui.allocate_rect(rect, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, format!("{} response curve: stick input against NEO output", axis.name())));
    let painter = ui.painter().with_clip_rect(rect);
    let at = |x: f64, y: f64| Pos2::new(rect.left() + side * (120.0 + x as f32 * 110.0) / 240.0, rect.top() + side * (120.0 - y as f32 * 110.0) / 240.0);
    painter.rect(rect, CornerRadius::same(theme::RADIUS), theme::GROUND, Stroke::new(1.0, theme::LINE_2), StrokeKind::Inside);
    let band = Rect::from_two_pos(at(-tuning.deadzone, 1.0), at(tuning.deadzone, -1.0));
    painter.rect_filled(band, CornerRadius::ZERO, tint(theme::WARN, 0.18));
    for v in [-0.5, 0.5] {
        painter.line_segment([at(v, -1.0), at(v, 1.0)], Stroke::new(1.0, theme::SUNK));
        painter.line_segment([at(-1.0, v), at(1.0, v)], Stroke::new(1.0, theme::SUNK));
    }
    painter.line_segment([at(0.0, -1.0), at(0.0, 1.0)], Stroke::new(1.0, theme::LINE));
    painter.line_segment([at(-1.0, 0.0), at(1.0, 0.0)], Stroke::new(1.0, theme::LINE));
    painter.extend(Shape::dashed_line(&[at(-1.0, -1.0), at(1.0, 1.0)], Stroke::new(1.0, theme::LINE_2), 4.0, 4.0));
    let points: Vec<Pos2> = previews::curve(tuning).into_iter().map(|(x, y)| at(x, y)).collect();
    painter.add(PathShape::line(points, Stroke::new(2.5, theme::ACCENT)));
    let output = shape(input, tuning);
    painter.extend(Shape::dashed_line(&[at(input, -1.0), at(input, output)], Stroke::new(1.0, Color32::from_white_alpha(89)), 2.0, 3.0));
    painter.circle(at(input, output), 6.0 * side / 240.0, theme::WHITE, Stroke::new(2.0, theme::ACCENT));
    painter.text(rect.left_top() + Vec2::new(side * 0.075, side * 0.1), Align2::LEFT_BOTTOM, "NEO OUT", mono(10.0), theme::DIM);
    painter.text(rect.right_bottom() - Vec2::new(side * 0.075, side * 0.06), Align2::RIGHT_BOTTOM, "STICK IN", mono(10.0), theme::DIM);
}

/// The raw input of a flight axis: live, or the test input for the tuned axis.
fn input(inputs: &Inputs, s: &Settings, axis: Axis) -> f64 {
    match (inputs.monitor, axis) {
        (Some(monitor), Axis::Roll) => monitor.roll,
        (Some(monitor), Axis::Pitch) => monitor.pitch,
        (Some(monitor), Axis::Yaw) => monitor.yaw,
        (None, axis) if axis == s.axis => s.test_input,
        (None, _) => 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn axes(ui: &mut Ui, full: Rect, s: &mut Settings, inputs: &Inputs, defaults: &Config, compact: bool, requests: &mut Vec<Request>) {
    let weights = if compact { [0.9, 1.15, 1.0] } else { [0.95, 1.15, 1.0] };
    let cols = columns(full, &weights, 12.0);
    let live = inputs.monitor.is_some();
    let config = s.draft.clone().expect("draft");
    let axis = s.axis;
    let name = axis.name();
    panel(ui, cols[0], |ui| {
        live_head(ui, &format!("{name} response"), live);
        let below = if live { 58.0 } else { 58.0 + 72.0 };
        let room = ui.allocate_exact_size(Vec2::new(ui.available_width(), (ui.available_height() - below).max(80.0)), Sense::hover()).0;
        let x = input(inputs, s, axis);
        curve_plot(ui, room, &config, axis, x);
        let y = shape(x, axis.tuning(&config));
        grid(ui, vec![(format!("{name} \u{b7} {}", if live { "live" } else { "test" }), signed(x), false), ("NEO output".to_string(), signed(y), true)], 2, 48.0, |ui, (title, value, out)| read(ui, &title, &value, out));
        if !live {
            test_slider(ui, &mut s.test_input, (-100.0, 100.0), "Test stick input, percent", "Start emulation to see your device. Meanwhile, drag the test input.");
        }
    });
    panel(ui, cols[1], |ui| {
        let lower = name.to_lowercase();
        let targets: Vec<String> = axis.copy_targets().iter().map(|target| target.name().to_lowercase()).collect();
        let copy = format!("Copy {lower} to {}", targets.join(" and "));
        let reset = format!("Reset {lower}");
        let mut actions = Vec::new();
        panel_head(
            ui,
            |ui| {
                ui.label(widgets::h3(&format!("{name} settings")));
            },
            |ui| {
                if widgets::button(ui, &reset, &reset, Button::Quiet, true) {
                    actions.push(false);
                }
                if widgets::button(ui, &copy, &copy, Button::Quiet, true) {
                    actions.push(true);
                }
            },
        );
        for copy in actions {
            requests.extend(if copy { s.copy_axis() } else { s.reset_axis(defaults) });
        }
        ui.spacing_mut().item_spacing.y = 8.0;
        field(ui, s, Field::Deadzone, "Deadzone", Some("Stick travel ignored around centre."), "%", true, requests);
        field(ui, s, Field::Saturation, "Full deflection at", Some("Stick travel that already gives 100% output."), "%", false, requests);
        field(ui, s, Field::AntiDeadzone, "Anti-deadzone", Some("Smallest output past the deadzone, to beat the game\u{2019}s own."), "%", false, requests);
        field(ui, s, Field::Curve, "Curve", Some("Positive softens the centre for fine aim; negative sharpens it."), "%", false, requests);
        field(ui, s, Field::Smoothing, "Smoothing", Some("Filters jitter. More smoothing adds delay."), "%", false, requests);
        let delay = previews::lag(axis.tuning(s.draft.as_ref().expect("draft")).smoothing);
        let text = if delay == 0 { "No added delay".to_string() } else { format!("\u{2248} {delay} ms added delay") };
        ui.label(RichText::new(text).font(mono(11.5)).color(theme::SOFT));
    });
    panel(ui, cols[2], |ui| {
        live_head(ui, "All axes", live);
        ui.spacing_mut().item_spacing.y = 8.0;
        for other in Axis::ALL {
            let raw = input(inputs, s, other);
            let out = shape(raw, other.tuning(&config));
            let border = if other == axis { theme::ACCENT } else { theme::LINE_2 };
            egui::Frame::new().fill(theme::GROUND).stroke(Stroke::new(1.0, border)).corner_radius(3).inner_margin(egui::Margin::symmetric(9, 6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.horizontal(|ui| {
                    ui.label(cap(other.name()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(signed(out)).font(mono(11.0)).color(theme::ACCENT)));
                });
                bar(ui, raw, 8.0, RAW, theme::SUNK);
                bar(ui, out, 8.0, theme::ACCENT, theme::SUNK);
            });
        }
        legend(ui, &[("TCA raw", RAW), ("Sent to NEO", theme::ACCENT)]);
        divider(ui);
        ui.label(cap("Yaw source \u{b7} NEO Rz"));
        let sources = [YawSource::Twist, YawSource::Off, YawSource::LeverDifferential];
        let current = sources.iter().position(|source| *source == config.yaw_source).expect("a yaw source");
        if let Some(index) = seg(ui, &[("Twist", "Yaw from the twist", true), ("Off", "No yaw", true), ("Lever diff.", "Yaw from the lever difference", true)], current, None) {
            requests.extend(s.set_yaw_source(sources[index]));
        }
        ui.label(help(match config.yaw_source {
            YawSource::Twist => "The sidestick twist steers the rudder, through the yaw curve.",
            YawSource::Off => "No yaw input: useful if the twist is too easy to knock in combat.",
            YawSource::LeverDifferential => "Pushing one quadrant lever ahead of the other yaws towards the back lever, through the yaw curve. The throttle follows the average of both levers.",
        }));
        divider(ui);
        ui.label(cap("Invert"));
        let invert = config.invert.clone();
        grid(ui, vec![("Roll", invert.roll), ("Pitch", invert.pitch), ("Yaw", invert.yaw)], 3, 36.0, |ui, (title, on)| {
            if toggle(ui, on, title, false, ui.available_width()) {
                requests.extend(s.edit("Unsaved changes.", |config| match title {
                    "Roll" => config.invert.roll = !on,
                    "Pitch" => config.invert.pitch = !on,
                    _ => config.invert.yaw = !on,
                }));
            }
        });
    });
}

fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, theme::LINE));
}

/// A legend row: small swatches and their names.
fn legend(ui: &mut Ui, keys: &[(&str, Color32)]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (text, colour) in keys {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 8.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::ZERO, *colour);
            ui.label(RichText::new(*text).size(11.5).color(theme::MUTED));
            ui.add_space(8.0);
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Throttle

/// Diagonal amber stripes (the brake zone and the back-lever zone).
fn hatch(painter: &egui::Painter, rect: Rect) {
    let painter = painter.with_clip_rect(rect);
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        painter.line_segment([Pos2::new(x, rect.bottom()), Pos2::new(x + rect.height(), rect.top())], Stroke::new(3.0, tint(theme::WARN, 0.55)));
        x += 6.0;
    }
}

fn needle(painter: &egui::Painter, rail: Rect, at: f64) {
    let x = rail.left() + rail.width() * at.clamp(0.0, 1.0) as f32;
    let line = Rect::from_x_y_ranges(x - 1.5..=x + 1.5, rail.top() - 3.0..=rail.bottom() + 3.0);
    painter.rect_filled(line.expand(2.0), CornerRadius::same(2), Color32::from_white_alpha(40));
    painter.rect_filled(line, CornerRadius::ZERO, theme::WHITE);
}

/// The lever travel the throttle page shows: the live lever while emulating, else the test lever.
fn travel(inputs: &Inputs, s: &Settings, config: &Config) -> f64 {
    let index = usize::from(config.throttle_lever == Lever::Right);
    inputs.monitor.map_or(s.test_lever, |monitor| monitor.levers[index])
}

fn throttle(ui: &mut Ui, full: Rect, s: &mut Settings, inputs: &Inputs, requests: &mut Vec<Request>) {
    let cols = columns(full, &[1.25, 1.0], 12.0);
    let config = s.draft.clone().expect("draft");
    let t = &config.throttle;
    let detents = t.mode == ThrottleMode::Detents;
    let lever = travel(inputs, s, &config);
    let out = throttle_out(lever, t);
    let (area, text) = zone(out, t);
    let side = if config.throttle_lever == Lever::Right { "Right" } else { "Left" };
    panel(ui, cols[0], |ui| {
        tone_head(ui, &format!("{side} lever \u{b7} {} \u{2192} NEO throttle", if inputs.monitor.is_some() { "live" } else { "test" }), (text.clone(), previews::zone_tone(area)));
        ui.label(help(if detents {
            "ACE COMBAT throttle: centre cruises, forward boosts, back brakes. The quadrant detents mark where each band starts."
        } else {
            "Lever travel maps straight to the NEO throttle. Mid-travel is cruise."
        }));
        let (rail, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
        let painter = ui.painter();
        painter.rect(rail, CornerRadius::same(2), theme::GROUND, Stroke::new(1.0, theme::LINE_2), StrokeKind::Inside);
        let x = |v: f64| rail.left() + rail.width() * v.clamp(0.0, 1.0) as f32;
        let span = |from: f64, to: f64| Rect::from_x_y_ranges(x(from)..=x(to), rail.y_range());
        if detents {
            hatch(painter, span(0.0, t.idle));
            painter.rect_filled(span(t.idle, t.cruise - t.band), CornerRadius::ZERO, tint(theme::ACCENT, 0.08));
            painter.rect_filled(span(t.cruise - t.band, t.cruise + t.band), CornerRadius::ZERO, tint(theme::OK, 0.35));
            painter.vline(x(t.cruise - t.band), rail.y_range(), Stroke::new(1.0, theme::OK));
            painter.vline(x(t.cruise + t.band), rail.y_range(), Stroke::new(1.0, theme::OK));
            painter.rect_filled(span(t.cruise + t.band, t.boost), CornerRadius::ZERO, tint(theme::ACCENT, 0.08));
            painter.rect_filled(span(t.boost, 1.0), CornerRadius::ZERO, tint(theme::ACCENT, 0.3));
        } else {
            painter.rect_filled(span(0.0, 1.0), CornerRadius::ZERO, tint(theme::ACCENT, 0.08));
        }
        needle(painter, rail, lever);
        let (marks, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
        if detents {
            for (value, label, align) in [(t.idle, "Idle", Align2::LEFT_TOP), (t.cruise, "CL \u{b7} cruise", Align2::CENTER_TOP), (t.boost, "Boost", Align2::RIGHT_TOP)] {
                ui.painter().text(Pos2::new(x(value), marks.top()), align, label, mono(11.0), theme::MUTED);
            }
        }
        if inputs.monitor.is_none() {
            test_slider(ui, &mut s.test_lever, (0.0, 100.0), "Test lever travel, percent", "Start emulation to see your levers. Meanwhile, drag the test lever.");
        }
        ui.horizontal(|ui| {
            ui.label(cap("NEO throttle (Z)"));
            let word = if out < 0.0 {
                "brake"
            } else if out > 0.0 {
                "boost"
            } else {
                "cruise"
            };
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(format!("{} \u{b7} {word}", signed(out))).font(mono(11.5)).color(theme::SOFT).strong()));
        });
        bar(ui, out, 12.0, if out < 0.0 { theme::WARN } else { theme::ACCENT }, theme::GROUND);
        if detents {
            let capture = inputs.monitor.is_some();
            let cards = vec![(Field::Idle, "Idle (brake below)"), (Field::Cruise, "CL (cruise centre)"), (Field::Boost, "Boost from"), (Field::Band, "Cruise band \u{b1}")];
            grid(ui, cards, 4, 104.0, |ui, (field, title)| {
                card(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let label = ui.label(RichText::new(title).size(12.0).color(theme::INK).strong());
                    ui.horizontal(|ui| number_unit(ui, s, field, &label, "%", 64.0, requests));
                    if field != Field::Band && widgets::button(ui, "Capture", &format!("Capture {title}"), Button::Quiet, capture) {
                        requests.extend(s.capture_detent(field, lever));
                    }
                });
            });
            error(ui, s, Field::Idle);
            ui.horizontal(|ui| {
                legend(ui, &[("Brake", tint(theme::WARN, 0.55)), ("Cruise band", tint(theme::OK, 0.45)), ("Boost", tint(theme::ACCENT, 0.35))]);
                ui.label(help(&if capture { format!("Put the {} lever in a detent, then Capture.", side.to_lowercase()) } else { "Start emulation to capture the lever in its detents.".to_string() }));
            });
        }
        let rest = ui.available_rect_before_wrap();
        let foot = Rect::from_min_max(Pos2::new(rest.left(), (rest.bottom() - 104.0).max(rest.top())), rest.max);
        ui.scope_builder(UiBuilder::new().max_rect(foot).layout(Layout::top_down(Align::Min)), |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            divider(ui);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width() - 140.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new("Throttle lever").size(12.5).color(theme::INK).strong());
                    ui.label(help("Shares the throttle with the sidestick slider. The last one moved takes over."));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(index) = seg(ui, &[("Left", "Left lever", true), ("Right", "Right lever", true)], usize::from(config.throttle_lever == Lever::Right), None) {
                        requests.extend(s.edit("Unsaved changes.", |config| config.throttle_lever = [Lever::Left, Lever::Right][index]));
                    }
                });
            });
            if toggle(ui, config.invert.throttle, "Invert throttle", false, ui.available_width()) {
                let on = !config.invert.throttle;
                requests.extend(s.edit("Unsaved changes.", |config| config.invert.throttle = on));
            }
        });
    });    panel(ui, cols[1], |ui| {
        let rule = &config.split_high_g;
        tone_head(ui, "High-G from split levers", previews::lever_state(rule, inputs.levers));
        if toggle(ui, rule.enabled, "Hold NEO 5 (High-G) while one quadrant lever is forward and the other back", false, ui.available_width()) {
            requests.extend(s.set_split(!rule.enabled));
        }
        egui::Frame::new().fill(theme::GROUND).stroke(Stroke::new(1.0, theme::LINE_2)).corner_radius(3).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            match inputs.levers {
                None => {
                    ui.label(help("Lever positions appear here during emulation."));
                }
                Some(levers) => {
                    ui.label(help(&previews::lever_text(levers, rule)));
                    for (name, value) in [("Left lever", levers[0]), ("Right lever", levers[1])] {
                        ui.horizontal(|ui| {
                            ui.add_sized(Vec2::new(84.0, 16.0), egui::Label::new(RichText::new(name).size(12.0).color(theme::SOFT)));
                            let (rail, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 54.0, 16.0), Sense::hover());
                            let painter = ui.painter();
                            painter.rect(rail, CornerRadius::same(2), theme::GROUND, Stroke::new(1.0, theme::LINE_2), StrokeKind::Inside);
                            let x = |v: f64| rail.left() + rail.width() * v as f32;
                            hatch(painter, Rect::from_x_y_ranges(rail.left()..=x(rule.back), rail.y_range()));
                            painter.rect_filled(Rect::from_x_y_ranges(x(rule.forward)..=rail.right(), rail.y_range()), CornerRadius::ZERO, tint(theme::ACCENT, 0.22));
                            needle(painter, rail, value);
                            let colour = if value >= rule.forward {
                                theme::ACCENT
                            } else if value <= rule.back {
                                theme::WARN_INK
                            } else {
                                theme::SOFT
                            };
                            ui.label(RichText::new(format!("{}%", (value * 100.0).round())).font(mono(11.5)).color(colour).strong());
                        });
                    }
                }
            }
            legend(ui, &[("Back zone", tint(theme::WARN, 0.55)), ("Forward zone", tint(theme::ACCENT, 0.35))]);
        });
        let width = (ui.available_width() - 8.0) / 2.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (field, title, warn) in [(Field::SplitForward, "Forward lever at least", false), (Field::SplitBack, "Other lever at most", true)] {
                ui.allocate_ui(Vec2::new(width, 110.0), |ui| card(ui, |ui| self::field(ui, s, field, title, None, "%", warn, requests)));
            }
        });
        ui.label(help("Move both levers: the rails follow them against the thresholds above, even before you save. Either lever can be the forward one; while split, the throttle follows it."));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Camera & head

/// The camera output while the hat is held: the ramp, the long-press marker and the current hold.
fn ramp_graph(ui: &mut Ui, rect: Rect, config: &Config, held: Option<f64>) {
    let response = ui.allocate_rect(rect, Sense::hover());
    let strength = config.camera_strength;
    let camera = &config.camera;
    let label = format!(
        "Camera output while the hat is held: {}{}.",
        if camera.ramp_ms == 0 { format!("{}% at once", (strength * 100.0).round()) } else { format!("ramps to {}% over {} ms", (strength * 100.0).round(), camera.ramp_ms) },
        if camera.full_after_hold { format!(", full after {} ms", config.timing.long_ms) } else { String::new() }
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, &label));
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect(rect, CornerRadius::same(3), theme::GROUND, Stroke::new(1.0, theme::LINE_2), StrokeKind::Inside);
    let x = |ms: f64| rect.left() + rect.width() * (10.0 + ms.min(f64::from(RAMP_SPAN_MS)) / f64::from(RAMP_SPAN_MS) * 280.0) as f32 / 300.0;
    let y = |value: f64| rect.top() + rect.height() * (88.0 - value * 76.0) as f32 / 100.0;
    let base = y(0.0);
    if let Some(ms) = held {
        painter.rect_filled(Rect::from_x_y_ranges(x(0.0)..=x(ms), y(1.05)..=base), CornerRadius::ZERO, tint(theme::OK, 0.12));
    }
    painter.hline(rect.x_range(), base, Stroke::new(1.0, theme::LINE));
    if camera.full_after_hold {
        let long = x(f64::from(config.timing.long_ms));
        painter.extend(Shape::dashed_line(&[Pos2::new(long, y(1.05)), Pos2::new(long, base)], Stroke::new(1.0, theme::LINE_2), 3.0, 3.0));
        painter.text(Pos2::new(long + 4.0, base - 6.0), Align2::LEFT_BOTTOM, "LONG PRESS", mono(9.0), theme::DIM);
    }
    let points: Vec<Pos2> = previews::ramp(camera, strength, config.timing.long_ms).into_iter().map(|(ms, value)| Pos2::new(x(f64::from(ms)), y(value))).collect();
    painter.add(PathShape::line(points, Stroke::new(2.5, theme::ACCENT)));
    if let Some(ms) = held {
        painter.vline(x(ms), y(1.05)..=base, Stroke::new(2.0, theme::OK));
    }
}

/// Where the head points and where that puts the camera, in a square with the deadzone band.
fn head_view(ui: &mut Ui, size: f32, config: &Config, sample: Option<[f64; 2]>) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let tracking = &config.head_tracking;
    let [x, y] = sample.map_or([0.0, 0.0], |sample| head_camera(sample, tracking));
    let label = if sample.is_some() { format!("Camera at {} across, {} down", signed(x), signed(y)) } else { "No head-tracking data".to_string() };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, &label));
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(theme::RADIUS), theme::GROUND, Stroke::new(1.0, theme::LINE_2), StrokeKind::Inside);
    let scale = size / 180.0;
    let at = |dx: f64, dy: f64| rect.center() + Vec2::new(dx as f32 * 80.0 * scale, dy as f32 * 80.0 * scale);
    painter.line_segment([at(0.0, -1.0), at(0.0, 1.0)], Stroke::new(1.0, theme::LINE));
    painter.line_segment([at(-1.0, 0.0), at(1.0, 0.0)], Stroke::new(1.0, theme::LINE));
    let band = tracking.tuning.deadzone;
    painter.rect_filled(Rect::from_two_pos(at(-band, -band), at(band, band)), CornerRadius::ZERO, tint(theme::WARN, 0.18));
    painter.circle(at(x, y), 7.0 * scale, theme::WHITE, Stroke::new(2.0, theme::ACCENT));
}

fn camera(ui: &mut Ui, full: Rect, s: &mut Settings, inputs: &Inputs, requests: &mut Vec<Request>) {
    let cols = columns(full, &[1.0, 1.0], 12.0);
    let config = s.draft.clone().expect("draft");
    let held = s.hat_since.map(|since| (inputs.now - since).max(0.0));
    panel(ui, cols[0], |ui| {
        tone_head(ui, "Camera \u{b7} hat \u{2192} NEO mini-stick", previews::hat_state(inputs.saved_hat, held));
        let width = (ui.available_width() - 10.0) / 2.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.allocate_ui(Vec2::new(width, 90.0), |ui| field(ui, s, Field::CameraStrength, "Strength", Some("How far the hat moves the mini-stick."), "%", false, requests));
            ui.allocate_ui(Vec2::new(width, 90.0), |ui| field(ui, s, Field::CameraRamp, "Ramp", Some("Time to full strength. 0 is instant."), "ms", false, requests));
        });
        let below = 36.0 + 10.0 + 36.0 + 10.0 + 40.0 + 10.0 + 34.0 + 30.0;
        let height = (ui.available_height() - below).clamp(70.0, 240.0);
        let (graph, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        ramp_graph(ui, graph, &config, held);
        if toggle(ui, config.camera.full_after_hold, "Full strength after a long press: hold the hat to look all the way round", false, ui.available_width()) {
            let on = !config.camera.full_after_hold;
            requests.extend(s.edit("Unsaved changes.", |config| config.camera.full_after_hold = on));
        }
        ui.horizontal(|ui| {
            ui.label(cap("Source"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(index) = seg(ui, &[("Hat", "Camera from the hat", true), ("Hat + spare lever", "Camera from the hat and the spare lever", true)], usize::from(config.camera.lever), None) {
                    requests.extend(s.set_camera_lever(index == 1));
                }
            });
        });
        let spare = if config.throttle_lever == Lever::Right { "left" } else { "right" };
        ui.label(help(&if config.camera.lever {
            format!("The {spare} quadrant lever (the one that is not the throttle) pans the camera left and right; centre it to look ahead. The hat still works and wins while pressed.")
        } else {
            "Only the hat moves the camera.".to_string()
        }));
        let camera_axes = [("Camera X", 3), ("Camera Y", 4)].map(|(title, index)| (title, inputs.monitor.map_or(0.0, |monitor| axis_value(monitor.axes[index]))));
        grid(ui, camera_axes.to_vec(), 2, 30.0, |ui, (title, value)| {
            ui.horizontal(|ui| {
                ui.label(cap(title));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(signed(value)).font(mono(12.0)).color(theme::ACCENT).strong()));
            });
            bar(ui, value, 8.0, theme::ACCENT, theme::GROUND);
        });
    });
    panel(ui, cols[1], |ui| {
        let tracking = &config.head_tracking;
        let sample = inputs.monitor.and_then(|monitor| monitor.head);
        let (text, tone) = previews::head_state(tracking, sample, inputs.emulating);
        tone_head(ui, "Head tracking \u{b7} OpenTrack", (text, tone));
        if toggle(ui, tracking.enabled, "Look around with your head: OpenTrack moves the NEO camera", false, ui.available_width()) {
            let on = !tracking.enabled;
            requests.extend(s.edit("Unsaved changes.", |config| config.head_tracking.enabled = on));
        }
        let size = (ui.available_height() - 180.0).clamp(120.0, 190.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            head_view(ui, size, &config, sample);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                let [x, y] = sample.map_or([0.0, 0.0], |sample| head_camera(sample, tracking));
                let reads = [("Head yaw", sample.map_or("\u{2014}".to_string(), |s| degrees(s[0]))), ("Head pitch", sample.map_or("\u{2014}".to_string(), |s| degrees(s[1])))];
                grid(ui, reads.to_vec(), 2, 48.0, |ui, (title, value)| read(ui, title, &value, false));
                read(ui, "Camera X / Y", &sample.map_or("\u{2014}".to_string(), |_| format!("{} / {}", signed(x), signed(y))), true);
                let toggles = vec![("Invert L / R", tracking.invert_yaw, true), ("Invert U / D", tracking.invert_pitch, false)];
                grid(ui, toggles, 2, 36.0, |ui, (title, on, yaw)| {
                    if toggle(ui, on, title, false, ui.available_width()) {
                        requests.extend(s.edit("Unsaved changes.", |config| {
                            if yaw {
                                config.head_tracking.invert_yaw = !on;
                            } else {
                                config.head_tracking.invert_pitch = !on;
                            }
                        }));
                    }
                });
            });
        });
        let cards = vec![
            (Field::HeadPort, "UDP port", ""),
            (Field::HeadYaw, "Full look L / R", "\u{b0}"),
            (Field::HeadPitch, "Full look U / D", "\u{b0}"),
            (Field::HeadTimeout, "Hat back after", "ms"),
            (Field::HeadDeadzone, "Deadzone", "%"),
            (Field::HeadCurve, "Curve", "%"),
        ];
        grid(ui, cards, 3, 46.0, |ui, (field, title, unit)| number_card(ui, s, field, (title, unit), if unit.is_empty() || unit == "ms" { 72.0 } else { 56.0 }, requests));
        error(ui, s, Field::HeadPort);
        ui.label(help(&format!(
            "OpenTrack output: UDP over network, remote IP 127.0.0.1, port {}. The hat wins while pressed. The port and on/off apply when emulation starts.",
            s.draft.as_ref().expect("draft").head_tracking.port
        )));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Buttons & helpers

/// A static chip: a chosen control ("S5 · Sidestick button 5"), or Off; green while held.
fn static_chip(ui: &mut Ui, key: Option<&String>, held: bool) {
    let (text, border, fill, ink) = match key {
        Some(key) if held => (mapping::short(key), theme::OK, tint(theme::OK, 0.15), theme::OK_INK),
        Some(key) => (mapping::short(key), theme::ACCENT, tint(theme::ACCENT, 0.1), theme::ACCENT),
        None => ("Off".to_string(), theme::LINE_2, theme::RAISE, theme::MUTED),
    };
    let galley = ui.painter().layout_no_wrap(text.clone(), mono(11.0), ink);
    let (rect, response) = ui.allocate_exact_size(Vec2::new((galley.size().x + 14.0).max(28.0), 28.0), Sense::hover());
    let label = key.map_or("Off".to_string(), |key| mapping::label(key));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &label));
    ui.painter().rect(rect, CornerRadius::same(3), fill, Stroke::new(1.0, border), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, ink);
}

/// The prompt shown while a control is being picked by pressing it.
fn capture_well(ui: &mut Ui, s: &Settings, kind: Kind, emulating: bool) {
    if s.capture.as_ref().map(|capture| capture.kind) != Some(kind) {
        return;
    }
    let text = match (emulating, kind) {
        (false, _) => "Start emulation, then press the controls on the TCA.",
        (true, Kind::Precision) => "Press the TCA control to hold for precision aiming.",
        (true, Kind::Shift) => "Press the TCA control to hold for the shift layer. Its own mapping is not sent while it is the shift control.",
        (true, _) => "Press and release the controls for the combo (up to 4), then choose Done.",
    };
    widgets::strip(ui, text, 32.0, (theme::LINE_2, theme::GROUND, theme::MUTED, theme::WARN));
}

/// Precision or shift: the chosen control, Press a control… (Cancel while picking) and Clear.
fn single(ui: &mut Ui, s: &mut Settings, kind: Kind, inputs: &Inputs, requests: &mut Vec<Request>) {
    let key = s.single(kind).cloned();
    let picking = s.capture.as_ref().is_some_and(|capture| capture.kind == kind);
    let name = if kind == Kind::Precision { "precision" } else { "shift" };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(cap("Hold"));
        static_chip(ui, key.as_ref(), key.as_ref().is_some_and(|key| inputs.held.contains(key)));
        if let Some(key) = &key {
            ui.label(RichText::new(mapping::label(key)).size(12.0).color(theme::SOFT));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::button(ui, "Clear", &format!("Clear the {name} control"), Button::Quiet, key.is_some()) {
                requests.extend(s.clear_single(kind));
            }
            let pick = if picking { "Cancel" } else { "Press a control\u{2026}" };
            if widgets::button(ui, pick, &format!("{pick} ({name})"), Button::Quiet, true) {
                s.start_capture(kind);
            }
        });
    });
    capture_well(ui, s, kind, inputs.emulating);
}

/// A combo row: its controls with + between them (or Off), Record combo… / Done, and Clear.
fn combo(ui: &mut Ui, s: &mut Settings, kind: Kind, title: &str, inputs: &Inputs, requests: &mut Vec<Request>) {
    let recording = s.capture.as_ref().filter(|capture| capture.kind == kind).map(|capture| capture.keys.clone());
    let keys = recording.clone().unwrap_or_else(|| kind.combo(s.draft.as_ref().expect("draft")).clone());
    let saved_any = !kind.combo(s.draft.as_ref().expect("draft")).is_empty();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (caption, _) = ui.allocate_exact_size(Vec2::new(96.0, 28.0), Sense::hover());
        ui.painter().text(caption.left_center(), Align2::LEFT_CENTER, title.to_uppercase(), mono(11.0), theme::DIM);
        if keys.is_empty() {
            static_chip(ui, None, false);
        }
        for (index, key) in keys.iter().enumerate() {
            if index > 0 {
                ui.label(RichText::new("+").font(mono(11.0)).color(theme::DIM));
            }
            static_chip(ui, Some(key), inputs.held.contains(key));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::button(ui, "Clear", &format!("Clear the {} combo", kind.text()), Button::Quiet, saved_any) {
                requests.extend(s.clear_combo(kind));
            }
            let record = if recording.is_some() { "Done" } else { "Record combo\u{2026}" };
            if widgets::button(ui, record, &format!("{record} ({})", kind.text()), Button::Quiet, true) {
                requests.extend(s.record_combo(kind));
            }
        });
    });
    capture_well(ui, s, kind, inputs.emulating);
}

/// A timeline lane: its name and its spans (from, to, colour) as fractions of the lane.
type Lane = (String, Vec<(f32, f32, Color32)>);

/// The press timeline: the latest control's presses over the last 3 seconds, and what each reads as.
fn presses(ui: &mut Ui, s: &Settings, config: &Config, now: f64) {
    let latest = s.latest();
    let (key, mine) = latest.map_or((None, Vec::new()), |(key, presses)| (Some(key.as_str()), presses));
    let timing = &config.timing;
    let text = previews::press_text(key, &mine, timing, now);
    let at = |t: f64| (((t - now + PRESS_SPAN) / PRESS_SPAN).clamp(0.0, 1.0)) as f32;
    let reads = gestures(&mine, timing, now);
    let response = egui::Frame::new().fill(theme::SUNK).stroke(Stroke::new(1.0, theme::LINE)).corner_radius(3).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 5.0;
        let lanes: [Lane; 2] = [
            (
                key.map_or("Pressed".to_string(), mapping::short),
                mine.iter().map(|press| (at(press.start), at(press.end.unwrap_or(now)), RAW)).chain(mine.last().filter(|last| last.start + f64::from(timing.long_ms) <= now).map(|last| {
                    let mark = at(last.start + f64::from(timing.long_ms));
                    (mark, mark + 0.004, theme::WARN)
                })).collect(),
            ),
            ("Reads as".to_string(), reads.iter().map(|(kind, from, to)| (at(*from), at(to.min(now)), if *kind == Gesture::Tap { theme::ACCENT } else { ADDED })).collect()),
        ];
        for (title, spans) in lanes {
            ui.horizontal(|ui| {
                let (label, _) = ui.allocate_exact_size(Vec2::new(84.0, 16.0), Sense::hover());
                ui.painter().text(label.left_center(), Align2::LEFT_CENTER, &title, mono(11.0), theme::MUTED);
                let (lane, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                ui.painter().rect(lane, CornerRadius::same(2), theme::GROUND, Stroke::new(1.0, theme::LINE_2), StrokeKind::Inside);
                for (from, to, colour) in spans {
                    let span = Rect::from_x_y_ranges(lane.left() + lane.width() * from..=lane.left() + lane.width() * to.max(from + 0.002), lane.y_range());
                    ui.painter().rect_filled(span.intersect(lane), CornerRadius::ZERO, colour);
                }
            });
        }
    });
    response.response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, &text));
    ui.label(RichText::new(text).font(mono(11.5)).color(theme::SOFT));
}

fn helpers(ui: &mut Ui, full: Rect, s: &mut Settings, inputs: &Inputs, requests: &mut Vec<Request>) {
    let config = s.draft.clone().expect("draft");
    let monitor = inputs.monitor;
    let halves = columns(full, &[1.0, 1.0], 12.0);
    // Rows: precision and shift, combos and safe start, then the timing panel across both columns.
    let tops = [0.0, 168.0, 336.0];
    let heights = [156.0, 156.0];
    let rect = |column: usize, row: usize| Rect::from_min_size(Pos2::new(halves[column].left(), full.top() + tops[row]), Vec2::new(halves[column].width(), heights[row]));
    panel(ui, rect(0, 0), |ui| {
        tone_head(ui, "Precision aiming", previews::precision_state(&config, monitor));
        single(ui, s, Kind::Precision, inputs, requests);
        field(ui, s, Field::PrecisionScale, "Stick scale while held", None, "%", false, requests);
    });
    panel(ui, rect(1, 0), |ui| {
        tone_head(ui, "Shift layer", previews::shift_state(&config, monitor));
        single(ui, s, Kind::Shift, inputs, requests);
        ui.label(help("While held, other controls use their Shift mappings (Mapping \u{203a} Shift). Its own mapping is not sent."));
    });
    panel(ui, rect(0, 1), |ui| {
        tone_head(ui, "Combos", previews::combo_state(&config, inputs.held));
        combo(ui, s, Kind::Pause, "Pause input", inputs, requests);
        combo(ui, s, Kind::Next, "Next profile", inputs, requests);
    });
    panel(ui, rect(1, 1), |ui| {
        tone_head(ui, "Safe start", previews::safe_state(&config, monitor));
        if toggle(ui, config.safe_start, "Hold the throttle at centre until the active lever or slider reaches it", false, ui.available_width()) {
            let on = !config.safe_start;
            requests.extend(s.edit("Unsaved changes.", |config| config.safe_start = on));
        }
        ui.label(help("Shows armed once your throttle has passed centre this session."));
    });
    let timing = Rect::from_min_max(Pos2::new(full.left(), full.top() + tops[2]), full.max);
    panel(ui, timing, |ui| {
        live_head(ui, "Timing \u{b7} your presses", monitor.is_some());
        let cards = vec![(Field::LongPress, "Long press"), (Field::DoubleTap, "Double tap window"), (Field::Repeat, "Repeat every"), (Field::Pulse, "Pulse length")];
        grid(ui, cards, 4, 46.0, |ui, (field, title)| number_card(ui, s, field, (title, "ms"), 64.0, requests));
        error(ui, s, Field::LongPress);
        presses(ui, s, s.draft.as_ref().expect("draft"), inputs.now);
    });
}
