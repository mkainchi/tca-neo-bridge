//! The Mapping page: TCA diagrams beside the NEO schematic. Selecting a TCA control lights its NEO targets; clicking
//! NEO targets builds its new mapping, which is recorded (saved) at once. With nothing selected, a NEO target shows
//! what feeds it.
use super::widgets::{self, cap, chip, help, mono, panel, panel_head, seg, strip, ChipState, Kind};
use crate::diagrams::{self, Diagram, Hand};
use crate::map::{Focus, Map, Side, HELP_INSPECTING, HELP_NOTHING, HELP_SELECTED};
use crate::mapping::{self, current, describe, diff, mode_card, mode_help, mode_label, mode_pulses, neo_action, neo_name, short, sources_for, Layer, Neo, Slot, HAT_SOURCES, MODES};
use crate::theme::{self, tint};
use crate::worker::Request;
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Role, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo};
use neo_common::config::{Config, HatMode};

pub const PLATE: Color32 = Color32::from_rgb(0xe2, 0xe8, 0xee);
const PLATE_GRID: Color32 = Color32::from_rgba_premultiplied(1, 3, 4, 15);
/// Kept targets' ring, added targets' ring, removed targets' dashes.
const KEPT: Color32 = Color32::from_rgb(0x0e, 0xa5, 0xe9);
const ADDED: Color32 = Color32::from_rgb(0xfa, 0xcc, 0x15);
const REMOVED: Color32 = Color32::from_rgb(0xe1, 0x1d, 0x48);

/// How a hotspot shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    Plain,
    Held,
    Feed,
    Selected,
    Kept,
    Added,
    Removed,
}

/// Column widths: TCA, side panel (the NEO column takes the rest).
fn columns(compact: bool) -> (f32, f32) {
    if compact {
        (300.0, 330.0)
    } else {
        (360.0, 380.0)
    }
}

/// The page body under its head; returns the requests to send.
pub fn page(ui: &mut Ui, saved: &Config, map: &mut Map, compact: bool) -> Vec<Request> {
    let mut requests = Vec::new();
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let (tca, side) = columns(compact);
    let gap = 12.0;
    let left = Rect::from_min_size(full.min, Vec2::new(tca, full.height()));
    let right = Rect::from_min_max(Pos2::new(full.right() - side, full.top()), full.max);
    let middle = Rect::from_min_max(Pos2::new(left.right() + gap, full.top()), Pos2::new(right.left() - gap, full.bottom()));
    panel(ui, left, |ui| tca_panel(ui, saved, map, compact));
    panel(ui, middle, |ui| neo_panel(ui, saved, map, &mut requests));
    panel(ui, right, |ui| side_panel(ui, saved, map, &mut requests));
    requests
}

/// The page head's right-hand side: the Normal / Shift layer switch.
pub fn layer_switch(ui: &mut Ui, saved: &Config, map: &mut Map) {
    let shift = saved.shift.as_deref().map(|key| format!("Shift \u{b7} {}", short(key)));
    let options = [(Layer::Normal, "Normal", "Normal layer", true), (Layer::Shift, shift.as_deref().unwrap_or("Shift"), "Shift layer", shift.is_some())];
    let response = ui.scope(|ui| seg(ui, &options, map.layer, Some(Layer::Shift)));
    if let Some(layer) = response.inner {
        map.set_layer(saved, layer);
    }
    if shift.is_none() {
        response.response.on_hover_text("Choose a shift control on the Buttons & helpers page");
    }
}

/// A rectangle of `aspect` (width / height) as large as fits, centred in `room`.
pub fn fit(room: Rect, aspect: f32) -> Rect {
    let size = if room.width() / room.height() > aspect { Vec2::new(room.height() * aspect, room.height()) } else { Vec2::new(room.width(), room.width() / aspect) };
    Rect::from_center_size(room.center(), size)
}

/// A diagram on its light measuring plate (a faint 16-point grid), fitted into `room`; returns the plate.
fn plate(ui: &mut Ui, room: Rect, diagram: &Diagram) -> Rect {
    let rect = fit(room, diagram.aspect());
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, CornerRadius::same(3), PLATE);
    let mut x = rect.left() + 16.0;
    while x < rect.right() {
        painter.vline(x, rect.y_range(), Stroke::new(1.0, PLATE_GRID));
        x += 16.0;
    }
    let mut y = rect.top() + 16.0;
    while y < rect.bottom() {
        painter.hline(rect.x_range(), y, Stroke::new(1.0, PLATE_GRID));
        y += 16.0;
    }
    egui::Image::new(diagram.image.clone()).paint_at(ui, rect);
    rect
}

/// One hotspot: an invisible button over a numbered callout, ringed by its state.
fn hotspot(ui: &mut Ui, id: egui::Id, (centre, size): (Pos2, f32), square: bool, label: &str, state: Spot, enabled: bool) -> bool {
    let rect = Rect::from_center_size(centre, Vec2::splat(size));
    let response = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, matches!(state, Spot::Selected | Spot::Kept | Spot::Added), label));
    let painter = ui.painter();
    let radius = if square { 3.0 } else { size / 2.0 };
    let ring = |color: Color32, width: f32, fill: Color32| {
        painter.rect(rect, CornerRadius::same(radius as u8), fill, Stroke::new(width, color), StrokeKind::Outside);
    };
    let glow = |color: Color32| {
        for step in 1..=4 {
            let spread = step as f32 * 2.0;
            painter.rect_stroke(rect.expand(3.0 + spread), CornerRadius::same((radius + 3.0 + spread) as u8), Stroke::new(2.0, color.gamma_multiply(0.5 / step as f32)), StrokeKind::Outside);
        }
    };
    match state {
        Spot::Plain => {}
        Spot::Held => ring(theme::OK, 3.0, tint(theme::OK, 0.35)),
        Spot::Feed => ring(theme::ACCENT, 3.0, tint(theme::ACCENT, 0.25)),
        Spot::Selected => {
            glow(theme::ACCENT);
            ring(theme::ACCENT, 3.0, Color32::TRANSPARENT);
        }
        Spot::Kept => ring(KEPT, 3.0, tint(theme::ACCENT, 0.4)),
        Spot::Added => {
            glow(ADDED);
            ring(ADDED, 3.0, tint(theme::ACCENT, 0.4));
        }
        Spot::Removed => dashed(painter, rect.expand(2.0), radius, REMOVED),
    }
    if enabled && response.hovered() && state != Spot::Removed {
        painter.rect_stroke(rect.expand(2.0), CornerRadius::same((radius + 2.0) as u8), Stroke::new(2.0, Color32::from_rgba_unmultiplied(0, 140, 180, 178)), StrokeKind::Outside);
    }
    widgets::focus_ring(ui, &response, rect.expand(1.0));
    response.clicked()
}

/// A dashed outline (the removed targets' mark).
fn dashed(painter: &egui::Painter, rect: Rect, radius: f32, color: Color32) {
    let points: Vec<Pos2> = if radius >= rect.width() / 2.0 - 2.0 {
        (0..=32).map(|i| rect.center() + Vec2::angled(i as f32 / 32.0 * std::f32::consts::TAU) * rect.width() / 2.0).collect()
    } else {
        vec![rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()]
    };
    painter.extend(Shape::dashed_line(&points, Stroke::new(2.5, color), 4.0, 3.0));
}

/// How a TCA source shows: selected, feeding the inspected target, held, or plain.
fn source_state(map: &Map, feeds: &[String], key: &str) -> Spot {
    if map.selected.as_deref() == Some(key) {
        Spot::Selected
    } else if feeds.iter().any(|feed| feed == key) {
        Spot::Feed
    } else if map.held.iter().any(|held| held == key) {
        Spot::Held
    } else {
        Spot::Plain
    }
}

/// How a NEO target shows: in the saved mapping and the proposal (kept), only the proposal (added), only the saved
/// mapping (removed), or inspected.
pub fn neo_state(saved: &Config, map: &Map, neo: Neo) -> Spot {
    match (&map.selected, &map.proposal) {
        (Some(key), Some(proposal)) => match (map.mapping(saved, key).has(neo), proposal.has(neo)) {
            (true, true) => Spot::Kept,
            (false, true) => Spot::Added,
            (true, false) => Spot::Removed,
            (false, false) => Spot::Plain,
        },
        _ if map.reverse == Some(neo) => Spot::Selected,
        _ => Spot::Plain,
    }
}

/// A figure caption: a mono label, and on the right an optional note.
fn caption(ui: &mut Ui, rect: Rect, left: &str, right: Option<(&str, Color32)>) {
    let painter = ui.painter();
    painter.text(rect.left_center(), Align2::LEFT_CENTER, left.to_uppercase(), mono(11.0), theme::DIM);
    if let Some((text, color)) = right {
        painter.text(rect.right_center(), Align2::RIGHT_CENTER, text.to_uppercase(), mono(11.0), color);
    }
}
fn tca_panel(ui: &mut Ui, saved: &Config, map: &mut Map, compact: bool) {
    panel_head(
        ui,
        |ui| {
            ui.label(widgets::h3("TCA"));
        },
        |ui| {
            if let Some(hand) = seg(ui, &[(Hand::Left, "L", "Left-hand setup", true), (Hand::Right, "R", "Right-hand setup", true)], map.hand, None) {
                map.hand = hand;
            }
        },
    );
    let feeds = map.reverse.map(|neo| sources_for(saved, neo)).unwrap_or_default();
    // Two figures share the rest of the panel 56 : 44, each a caption above a room for its plate.
    let rest = ui.available_rect_before_wrap();
    ui.allocate_rect(rest, Sense::hover());
    let (caption_height, gap) = (16.0, 6.0);
    let rooms = rest.height() - 2.0 * (caption_height + gap) - 10.0;
    let stick_top = rest.top();
    let quad_top = stick_top + caption_height + gap + rooms * 0.56 + 10.0;
    let setup = if map.hand == Hand::Right { "Right-hand setup" } else { "Left-hand setup" };
    caption(ui, Rect::from_min_size(Pos2::new(rest.left(), stick_top), Vec2::new(rest.width(), caption_height)), "Sidestick", Some((setup, theme::ACCENT)));
    let room = Rect::from_min_size(Pos2::new(rest.left(), stick_top + caption_height + gap), Vec2::new(rest.width(), rooms * 0.56));
    spots(ui, room, diagrams::stick(map.hand), map, &feeds, saved);
    caption(ui, Rect::from_min_size(Pos2::new(rest.left(), quad_top), Vec2::new(rest.width(), caption_height)), "Quadrant \u{b7} Eng 1&2", (!compact).then_some(("7 \u{b7} 8 = mode selector", theme::DIM)));
    let room = Rect::from_min_size(Pos2::new(rest.left(), quad_top + caption_height + gap), Vec2::new(rest.width(), rooms * 0.44));
    spots(ui, room, &diagrams::QUADRANT, map, &feeds, saved);
}
/// A TCA diagram and its source hotspots; a click selects the source.
fn spots(ui: &mut Ui, room: Rect, diagram: &Diagram, map: &mut Map, feeds: &[String], saved: &Config) {
    let plate = plate(ui, room, diagram);
    for (index, &(key, x, y, size)) in diagram.spots.iter().enumerate() {
        let circle = diagram.place(plate, (x, y, size));
        let state = source_state(map, feeds, key);
        if hotspot(ui, ui.scope_id().with(("tca-spot", key, index)), circle, false, &mapping::label(key), state, true) {
            map.select(saved, key);
        }
    }
}

fn legend(ui: &mut Ui) {
    for (text, spot) in [("Selected", Spot::Selected), ("Removed", Spot::Removed), ("Added", Spot::Added), ("Kept", Spot::Kept)] {
        ui.label(RichText::new(text).size(11.0).color(theme::SOFT));
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(11.0), Sense::hover());
        let painter = ui.painter();
        let swatch = Color32::from_rgb(0x3e, 0x8f, 0xb0);
        match spot {
            Spot::Kept => painter.circle(rect.center(), 5.5, swatch, Stroke::new(2.0, KEPT)),
            Spot::Added => painter.circle(rect.center(), 5.5, swatch, Stroke::new(2.0, ADDED)),
            Spot::Removed => {
                dashed(painter, rect, 5.5, REMOVED);
                continue;
            }
            _ => painter.circle(rect.center(), 5.5, Color32::TRANSPARENT, Stroke::new(2.0, theme::ACCENT)),
        };
    }
}

/// The NEO targets light up for the selection; with nothing selected, a click inspects what feeds a target.
fn neo_panel(ui: &mut Ui, saved: &Config, map: &mut Map, requests: &mut Vec<Request>) {
    panel_head(
        ui,
        |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 12.0), Sense::hover());
            let stroke = Stroke::new(1.8, theme::ACCENT);
            ui.painter().line(vec![rect.left_center() + Vec2::new(1.0, 0.0), rect.right_center() - Vec2::new(2.0, 0.0)], stroke);
            ui.painter().line(vec![Pos2::new(rect.right() - 7.0, rect.top() + 1.0), rect.right_center() - Vec2::new(2.0, 0.0), Pos2::new(rect.right() - 7.0, rect.bottom() - 1.0)], stroke);
            ui.label(widgets::h3("T.Flight Hotas NEO"));
        },
        legend,
    );
    let focus = map.focus();
    let colours = if focus == Focus::Nothing {
        (theme::LINE_2, theme::GROUND, theme::MUTED, theme::LINE_2)
    } else {
        (theme::ACCENT, theme::over(theme::PANEL, theme::ACCENT, 0.08), Color32::from_rgb(0xdd, 0xf7, 0xff), theme::ACCENT)
    };
    strip(ui, &map.context(saved), 28.0, colours);
    let rows = chip_rows(ui.available_width());
    let below = 32.0 + 10.0 + rows.iter().sum::<f32>() + 10.0 * rows.len() as f32;
    let room = ui.allocate_exact_size(Vec2::new(ui.available_width(), (ui.available_height() - below).max(80.0)), Sense::hover()).0;
    let plate = plate(ui, room, &diagrams::NEO);
    for &(key, x, y, size) in diagrams::NEO.spots {
        let circle = diagrams::NEO.place(plate, (x, y, size));
        let target = diagrams::neo_target(key);
        let label = target.map_or("NEO 16: on the hardware, not in the NEO report".to_string(), neo_name);
        let state = target.map_or(Spot::Plain, |neo| neo_state(saved, map, neo));
        if hotspot(ui, ui.scope_id().with(("neo-spot", key)), circle, key.starts_with("pov-"), &label, state, target.is_some()) {
            map.pick(target.expect("enabled hotspots are targets"));
        }
    }
    let active = !map.held.is_empty();
    let colours = if active { (tint(theme::OK, 0.5), theme::GROUND, theme::WHITE, theme::OK) } else { (theme::LINE_2, theme::GROUND, theme::MUTED, theme::OK) };
    strip(ui, &map.sending(), 32.0, colours);
    let mapped = saved.hat == HatMode::Mapped;
    let hats: Vec<String> = HAT_SOURCES.map(String::from).to_vec();
    chip_row(ui, "Hat drives", &hats, rows[0], map, saved, Some(mapped), requests);
    let detents: Vec<String> = (9..=16).map(|n| format!("q{n}")).collect();
    chip_row(ui, "Detents", &detents, rows[1], map, saved, None, requests);
    let addons: Vec<String> = (17..=31).map(|n| format!("q{n}")).collect();
    chip_row(ui, "Add-ons", &addons, rows[2], map, saved, None, requests);
}

/// Chips per line in a row this wide (after its caption): 28-point chips with 4 points between them.
fn per_line(width: f32) -> usize {
    (((width - 84.0 + 4.0) / 32.0).floor() as usize).max(1)
}

/// Heights of the three chip rows for a panel this wide (the add-ons wrap in narrow windows).
fn chip_rows(width: f32) -> [f32; 3] {
    let lines = 15usize.div_ceil(per_line(width)) as f32;
    [36.0, 28.0, 28.0 * lines + 4.0 * (lines - 1.0)]
}

/// A captioned row of source chips, right-aligned and wrapping in order; the hat row ends with its Camera / Mapped
/// switch.
#[allow(clippy::too_many_arguments)]
fn chip_row(ui: &mut Ui, title: &str, keys: &[String], height: f32, map: &mut Map, saved: &Config, hat: Option<bool>, requests: &mut Vec<Request>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter().text(Pos2::new(rect.left(), rect.top() + 14.0), Align2::LEFT_CENTER, title.to_uppercase(), mono(11.0), theme::DIM);
    let feeds = map.reverse.map(|neo| sources_for(saved, neo)).unwrap_or_default();
    let chips = Rect::from_min_max(Pos2::new(rect.left() + 84.0, rect.top()), rect.max);
    let mut top = rect.top();
    for (index, line) in keys.chunks(per_line(rect.width())).enumerate() {
        let line_rect = Rect::from_min_size(Pos2::new(chips.left(), top), Vec2::new(chips.width(), if hat.is_some() { 36.0 } else { 28.0 }));
        top += 32.0;
        ui.scope_builder(UiBuilder::new().max_rect(line_rect).layout(Layout::right_to_left(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            if let (Some(mapped), 0) = (hat, index) {
                let switch = seg(ui, &[(HatMode::Camera, "Camera", "Hat drives the camera", true), (HatMode::Mapped, "Mapped", "Hat mapped", true)], if mapped { HatMode::Mapped } else { HatMode::Camera }, None);
                requests.extend(switch.and_then(|mode| map.set_hat(saved, mode)));
                ui.add_space(4.0);
            }
            for key in line.iter().rev() {
                source_chip(ui, key, map, saved, &feeds, hat == Some(false));
            }
        });
    }
}

/// One source chip ("9", "Up"); a click selects the source. Hat chips are disabled while the hat drives the camera.
fn source_chip(ui: &mut Ui, key: &str, map: &mut Map, saved: &Config, feeds: &[String], hat_off: bool) {
    let (text, disabled) = match key.strip_prefix("hat-") {
        Some(direction) => (format!("{}{}", direction[..1].to_uppercase(), &direction[1..]), hat_off),
        None => (key[1..].to_string(), false),
    };
    let state = match source_state(map, feeds, key) {
        _ if disabled => ChipState::Disabled,
        Spot::Selected => ChipState::Selected,
        Spot::Feed => ChipState::Feed,
        Spot::Held => ChipState::Held,
        _ => ChipState::Off,
    };
    let label = if disabled { "Set the hat to Mapped controls to map it".to_string() } else { mapping::label(key) };
    if chip(ui, &text, &label, state).clicked() {
        map.select(saved, key);
    }
}
fn tab_id(ui: &Ui, side: Side) -> egui::Id {
    ui.scope_id().with(("side-tab", side as usize))
}

fn subtabs(ui: &mut Ui, map: &mut Map) {
    let count = format!("Log \u{b7} {}", map.labels.len());
    let tabs = [(Side::Selected, if map.focus() == Focus::Inspecting { "Selected NEO control" } else { "Selected control" }), (Side::Buttons, "NEO buttons"), (Side::Log, count.as_str())];
    let row = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (side, text) in tabs {
            let text = text.to_uppercase();
            let galley = ui.painter().layout_no_wrap(text.clone(), mono(11.0), theme::MUTED);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x + 20.0, 34.0), Sense::hover());
            let response = ui.interact(rect, tab_id(ui, side), Sense::click());
            let on = map.side == side;
            response.widget_info(|| WidgetInfo::selected(Role::Tab, true, on, &text));
            let ink = if on { theme::ACCENT } else if response.hovered() { theme::INK } else { theme::MUTED };
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, &text, mono(11.0), ink);
            if on {
                ui.painter().hline(rect.x_range(), rect.bottom() - 1.0, Stroke::new(2.0, theme::ACCENT));
            }
            widgets::focus_ring(ui, &response, rect.shrink(2.0));
            if response.clicked() {
                map.side = side;
            }
            // The tab follows the keyboard focus: arrow keys (egui's focus movement) and Tab switch tabs.
            if response.gained_focus() {
                map.side = side;
            }
        }
    });
    let rect = row.response.rect;
    ui.painter().hline(ui.max_rect().x_range(), rect.bottom(), Stroke::new(1.0, theme::LINE));
}

fn side_panel(ui: &mut Ui, saved: &Config, map: &mut Map, requests: &mut Vec<Request>) {
    subtabs(ui, map);
    match map.side {
        Side::Selected => selected(ui, saved, map, requests),
        Side::Buttons => buttons(ui, saved, map),
        Side::Log => log(ui, map, requests),
    }
}

fn boxed(ui: &mut Ui, fill: Color32, border: Color32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, border)).corner_radius(3).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

fn selected(ui: &mut Ui, saved: &Config, map: &mut Map, requests: &mut Vec<Request>) {
    let focus = map.focus();
    let actions = 30.0 + 22.0;
    let height = (ui.available_height() - actions).max(0.0);
    egui::ScrollArea::vertical().max_height(height).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 10.0;
        if map.layer == Layer::Shift {
            let shift = saved.shift.as_deref().map(mapping::label).unwrap_or_default();
            boxed(ui, theme::over(theme::PANEL, theme::WARN, 0.08), tint(theme::WARN, 0.5), |ui| {
                ui.label(RichText::new(format!("Shift layer: these mappings apply while {shift} is held. Controls without one keep their normal mapping.")).size(12.0).color(theme::WARN_INK));
            });
        }
        let (kind, name) = match (&map.selected, map.reverse) {
            (Some(key), _) => ("TCA source", mapping::label(key)),
            (None, Some(neo)) => ("NEO target", neo_name(neo)),
            _ => ("Selection", "Nothing selected".to_string()),
        };
        boxed(ui, Color32::from_rgb(0x16, 0x1d, 0x27), theme::LINE, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(cap(kind));
            ui.label(RichText::new(&name).font(egui::FontId::new(16.0, theme::display())).color(theme::ACCENT).strong());
        });
        if let (Some(key), Some(proposal)) = (map.selected.clone(), map.proposal.clone()) {
            slots(ui, saved, map, &key);
            let now = map.mapping(saved, &key);
            let changed = now != proposal;
            compare(ui, &[("Mapped now", map.shown(saved, &now, map.slot)), ("New mapping", if changed { describe(&proposal) } else { "Unchanged".into() })]);
            changes(ui, &diff(&now, &proposal));
            if map.slot == Slot::Press {
                behaviour(ui, map, &proposal);
            } else {
                gestures(ui, saved);
            }
        } else if let Some(neo) = map.reverse {
            let feeds = sources_for(saved, neo);
            let fed = if feeds.is_empty() { "No TCA control".to_string() } else { feeds.iter().map(|key| mapping::label(key)).collect::<Vec<_>>().join(", ") };
            compare(ui, &[("Fed by", fed)]);
        }
        ui.label(help(match focus {
            Focus::Selected => HELP_SELECTED,
            Focus::Inspecting => HELP_INSPECTING,
            Focus::Nothing => HELP_NOTHING,
        }));
    });
    let changed = map.changed(saved);
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.add_sized(Vec2::new(ui.available_width(), 16.0), egui::Label::new(RichText::new(map.note.as_deref().unwrap_or("")).font(mono(12.0)).color(theme::OK_INK)).truncate());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if widgets::button(ui, "Record mapping", "Record mapping", Kind::Primary, changed && !map.busy) {
                requests.extend(map.record(saved));
            }
            if widgets::button(ui, "Undo changes", "Undo changes", Kind::Plain, changed && !map.busy) {
                map.revert(saved);
            }
            if widgets::button(ui, "Deselect", "Deselect", Kind::Plain, focus != Focus::Nothing) {
                map.deselect();
            }
        });
    });
}

/// Press, long press and double tap: what each sends, and which one is being edited.
fn slots(ui: &mut Ui, saved: &Config, map: &mut Map, key: &str) {
    let width = (ui.available_width() - 10.0) / 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        for slot in Slot::ALL {
            let enabled = map.layer == Layer::Normal || slot == Slot::Press;
            let text = map.shown(saved, &current(saved, key, slot, map.layer), slot);
            let on = map.slot == slot;
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 40.0), if enabled { Sense::click() } else { Sense::hover() });
            response.widget_info(|| WidgetInfo::selected(Role::RadioButton, enabled, on, slot.label()));
            let fade = |color: Color32| if enabled { color } else { color.gamma_multiply(0.4) };
            let (fill, border, title) = if on { (theme::over(theme::PANEL, theme::ACCENT, 0.08), theme::ACCENT, theme::ACCENT) } else { (theme::GROUND, theme::LINE_2, theme::INK) };
            ui.painter().rect(rect, CornerRadius::same(3), fade(fill), Stroke::new(1.0, fade(border)), StrokeKind::Inside);
            widgets::focus_ring(ui, &response, rect);
            let painter = ui.painter().with_clip_rect(rect.shrink(1.0));
            painter.text(rect.left_top() + Vec2::new(8.0, 6.0), Align2::LEFT_TOP, slot.label(), mono(11.0), fade(title));
            painter.text(rect.left_top() + Vec2::new(8.0, 21.0), Align2::LEFT_TOP, text, mono(11.0), fade(theme::MUTED));
            if response.clicked() {
                map.set_slot(saved, slot);
            }
        }
    });
    let note = if map.layer == Layer::Shift { "The shift layer changes the press only; long press and double tap are the same in both layers." } else { map.slot.help() };
    ui.label(help(note));
}

fn compare(ui: &mut Ui, rows: &[(&str, String)]) {
    egui::Grid::new("compare").num_columns(2).spacing(Vec2::new(8.0, 4.0)).min_col_width(88.0).show(ui, |ui| {
        for (index, (title, value)) in rows.iter().enumerate() {
            ui.label(cap(title));
            ui.label(RichText::new(value).font(mono(12.0)).color(if index == 1 { theme::WHITE } else { theme::SOFT }));
            ui.end_row();
        }
    });
}

fn changes(ui: &mut Ui, list: &[(bool, String)]) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for (added, text) in list {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                if *added {
                    ui.painter().circle_stroke(rect.center(), 5.0, Stroke::new(2.0, ADDED));
                } else {
                    dashed(ui.painter(), rect, 5.0, REMOVED);
                }
                ui.label(RichText::new(text).font(mono(12.0)).color(theme::SOFT));
            });
        }
    });
}
/// A lane of the timeline: a label, then bars at (start, width) fractions.
fn lane(ui: &mut Ui, title: &str, label_width: f32, bars: &[(f32, f32, Color32)]) {
    ui.horizontal(|ui| {
        let (label, _) = ui.allocate_exact_size(Vec2::new(label_width, 12.0), Sense::hover());
        ui.painter().text(label.left_center(), Align2::LEFT_CENTER, title, mono(11.0), theme::MUTED);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 12.0), Sense::hover());
        ui.painter().rect(rect, CornerRadius::same(2), theme::GROUND, Stroke::new(1.0, theme::LINE_2), StrokeKind::Inside);
        for (start, width, color) in bars {
            let bar = Rect::from_min_size(Pos2::new(rect.left() + rect.width() * start, rect.top()), Vec2::new(rect.width() * width, rect.height())).intersect(rect);
            ui.painter().rect_filled(bar, CornerRadius::ZERO, *color);
        }
    });
}

const INPUT: Color32 = Color32::from_rgba_premultiplied(81, 89, 101, 140);

fn timeline(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let response = boxed_response(ui, add);
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, label));
}

fn boxed_response(ui: &mut Ui, add: impl FnOnce(&mut Ui)) -> egui::Response {
    egui::Frame::new().fill(Color32::from_rgb(0x16, 0x1d, 0x27)).stroke(Stroke::new(1.0, theme::LINE)).corner_radius(3).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 5.0;
        add(ui);
    })
    .response
}

/// The press's behaviour: five choices and a timeline of what the game receives.
fn behaviour(ui: &mut Ui, map: &mut Map, proposal: &mapping::Mapping) {
    ui.label(cap("Behaviour"));
    let width = (ui.available_width() - 12.0) / 3.0;
    egui::Frame::new().fill(theme::GROUND).stroke(Stroke::new(1.0, theme::LINE_2)).corner_radius(theme::RADIUS).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(3.0);
        egui::Grid::new("modes").spacing(Vec2::splat(3.0)).show(ui, |ui| {
            for (index, mode) in MODES.into_iter().enumerate() {
                let (title, detail) = mode_card(mode);
                let on = proposal.mode == mode;
                let (rect, response) = ui.allocate_exact_size(Vec2::new(width - 2.0, 38.0), Sense::click());
                response.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, on, mode_label(mode)));
                let (fill, ink, sub) = if on { (theme::ACCENT, theme::GROUND, theme::GROUND) } else { (Color32::TRANSPARENT, if response.hovered() { theme::INK } else { theme::MUTED }, theme::MUTED) };
                ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
                widgets::focus_ring(ui, &response, rect);
                ui.painter().text(rect.left_top() + Vec2::new(8.0, 5.0), Align2::LEFT_TOP, title, mono(11.0), ink);
                ui.painter().text(rect.left_top() + Vec2::new(8.0, 20.0), Align2::LEFT_TOP, detail, egui::FontId::proportional(11.0), sub);
                if response.clicked() {
                    map.set_mode(mode);
                }
                if index % 3 == 2 {
                    ui.end_row();
                }
            }
        });
    });
    let pulses: Vec<(f32, f32, Color32)> = mode_pulses(proposal.mode).iter().map(|(start, width)| (*start, *width, theme::ACCENT)).collect();
    timeline(ui, &format!("What the game receives with {}", mode_label(proposal.mode)), |ui| {
        lane(ui, "Control", 64.0, &[(0.15, 0.55, INPUT)]);
        lane(ui, "NEO", 64.0, &pulses);
    });
    ui.label(help(mode_help(proposal.mode)));
}

/// Long press and double tap: when a tap, a hold and a double tap are sent.
fn gestures(ui: &mut Ui, saved: &Config) {
    ui.label(cap("What the game receives"));
    timeline(ui, "A tap sends the press after the double-tap window; a hold sends the long press; two quick taps send the double tap", |ui| {
        lane(ui, "Tap", 84.0, &[(0.08, 0.06, INPUT), (0.32, 0.06, theme::ACCENT)]);
        lane(ui, "Hold", 84.0, &[(0.08, 0.60, INPUT), (0.28, 0.40, ADDED)]);
        lane(ui, "Double tap", 84.0, &[(0.08, 0.05, INPUT), (0.18, 0.05, INPUT), (0.18, 0.06, ADDED)]);
    });
    ui.label(help(&format!(
        "With a long press or double tap set, a tap is sent once the control is released and the double-tap window ({} ms) has passed, as a short press.",
        saved.timing.double_ms
    )));
}

/// "S1 · Q3" for the sources feeding a target, or a dash.
fn fed(saved: &Config, neo: Neo) -> String {
    let keys: Vec<String> = sources_for(saved, neo).iter().map(|key| short(key)).collect();
    if keys.is_empty() {
        "\u{2014}".into()
    } else {
        keys.join(" \u{b7} ")
    }
}

/// The NEO buttons table: each button's AC8 action and what feeds it; a click inspects or toggles it.
fn buttons(ui: &mut Ui, saved: &Config, map: &mut Map) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::Frame::new().stroke(Stroke::new(1.0, theme::LINE)).corner_radius(theme::RADIUS).show(ui, |ui| {
            ui.set_width(ui.available_width());
            row(ui, RowKind::Head, ("NEO", "ACTION", theme::DIM, "FED BY"), false, false);
            for n in 1..=15u8 {
                let (action, ink) = neo_action(n).map_or(("No AC8 action", theme::DIM), |action| (action, theme::INK));
                let active = map.reverse == Some(Neo::Button(n));
                let new = map.proposal.as_ref().is_some_and(|proposal| proposal.buttons.contains(&n));
                if row(ui, RowKind::Button(n), (&n.to_string(), action, ink, &fed(saved, Neo::Button(n))), active, new) {
                    map.pick(Neo::Button(n));
                }
            }
            let pov: Vec<String> = mapping::DIRECTIONS
                .iter()
                .map(|direction| (mapping::direction_name(*direction), fed(saved, Neo::Pov(*direction))))
                .filter(|(_, fed)| fed != "\u{2014}")
                .map(|(name, fed)| format!("{}: {fed}", name[..1].to_uppercase()))
                .collect();
            let pov = if pov.is_empty() { "\u{2014}".to_string() } else { pov.join("  ") };
            let active = matches!(map.reverse, Some(Neo::Pov(_)));
            let new = map.proposal.as_ref().is_some_and(|proposal| proposal.pov.is_some());
            row(ui, RowKind::Static, ("POV", "Hat, 4 directions \u{b7} pick an arrow on the diagram", theme::DIM, &pov), active, new);
            row(ui, RowKind::Inert, ("16", "On the hardware, not in the NEO report", theme::DIM, "\u{2014}"), false, false);
        });
        ui.add_space(12.0);
        ui.label(cap("NEO axes"));
        ui.add_space(6.0);
        egui::Grid::new("axes").num_columns(3).spacing(Vec2::new(8.0, 4.0)).min_col_width(56.0).show(ui, |ui| {
            for (axis, source, action) in mapping::axes(saved) {
                ui.label(RichText::new(axis).font(mono(11.5)).color(theme::WHITE).strong());
                ui.label(RichText::new(source).size(12.0).color(theme::SOFT));
                ui.label(RichText::new(action).font(mono(11.5)).color(theme::MUTED));
                ui.end_row();
            }
        });
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Head,
    Button(u8),
    Static,
    Inert,
}

/// One row of the NEO buttons table: (number, action, action colour, fed by); true when a button row was clicked.
fn row(ui: &mut Ui, kind: RowKind, (number, action, ink, fed): (&str, &str, Color32, &str), active: bool, new: bool) -> bool {
    let height = if kind == RowKind::Head { 26.0 } else { 30.0 };
    let button = matches!(kind, RowKind::Button(_));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), if button { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::selected(if button { Role::Button } else { Role::Row }, true, active, format!("NEO {number}: {action}, fed by {fed}")));
    let painter = ui.painter();
    let fill = match kind {
        RowKind::Head => Color32::from_rgb(0x16, 0x1d, 0x27),
        _ if active => theme::over(theme::PANEL, theme::ACCENT, 0.08),
        RowKind::Button(_) if response.hovered() => theme::RAISE,
        _ => Color32::TRANSPARENT,
    };
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    if active {
        painter.rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, theme::ACCENT), StrokeKind::Inside);
    }
    let rule = match kind {
        RowKind::Head => theme::LINE_2,
        RowKind::Inert => Color32::TRANSPARENT,
        _ => theme::LINE,
    };
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, rule));
    widgets::focus_ring(ui, &response, rect.shrink(2.0));
    let painter = ui.painter().with_clip_rect(rect);
    if kind == RowKind::Head {
        painter.text(Pos2::new(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, number, mono(11.0), theme::DIM);
        painter.text(Pos2::new(rect.left() + 60.0, rect.center().y), Align2::LEFT_CENTER, action, mono(11.0), theme::DIM);
        painter.text(Pos2::new(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, fed, mono(11.0), theme::DIM);
        return false;
    }
    let number_box = Rect::from_center_size(Pos2::new(rect.left() + 26.0, rect.center().y), Vec2::new(36.0, 20.0));
    let (fill, text) = if new { (theme::ACCENT, theme::GROUND) } else { (theme::RAISE, theme::INK) };
    let fade = if kind == RowKind::Inert { 0.5 } else { 1.0 };
    painter.rect(number_box, CornerRadius::same(2), fill.gamma_multiply(fade), Stroke::new(1.0, theme::LINE_2.gamma_multiply(fade)), StrokeKind::Inside);
    painter.text(number_box.center(), Align2::CENTER_CENTER, number, mono(11.0), text.gamma_multiply(fade));
    let fed_width = painter.layout_no_wrap(fed.to_string(), mono(11.5), theme::ACCENT).size().x;
    let action_area = Rect::from_min_max(Pos2::new(rect.left() + 60.0, rect.top()), Pos2::new(rect.right() - 16.0 - fed_width, rect.bottom()));
    painter.with_clip_rect(action_area).text(action_area.left_center(), Align2::LEFT_CENTER, action, egui::FontId::proportional(12.0), ink);
    painter.text(Pos2::new(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, fed, mono(11.5), theme::ACCENT);
    response.clicked()
}
/// The correction log, newest first; each entry can be removed (the mapping itself stays).
fn log(ui: &mut Ui, map: &mut Map, requests: &mut Vec<Request>) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if map.labels.is_empty() {
            let rect = ui.available_rect_before_wrap();
            let response = egui::Frame::new().inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.set_width(rect.width() - 20.0);
                ui.label(help("No corrections yet. Each recorded mapping is logged here and in label-map.json."));
            });
            dashed(ui.painter(), response.response.rect, 3.0, theme::LINE_2);
            return;
        }
        ui.spacing_mut().item_spacing.y = 5.0;
        for (index, entry) in map.labels.iter().enumerate().rev() {
            let (was, to) = entry.mapping.split_once(" \u{2192} ").unwrap_or((&entry.mapping, &entry.mapping));
            boxed(ui, Color32::from_rgb(0x16, 0x1d, 0x27), theme::LINE, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.set_max_width(ui.available_width() - 80.0);
                        ui.label(RichText::new(&entry.label).size(12.0).color(theme::WHITE).strong());
                        ui.label(RichText::new(format!("\u{2192} {to}")).font(mono(11.0)).color(theme::ACCENT));
                        ui.label(RichText::new(format!("was {was}")).font(mono(11.0)).color(theme::DIM));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if widgets::button(ui, "Remove", &format!("Remove {} from the correction log", entry.label), Kind::Quiet, true) {
                            requests.push(map.forget(index));
                        }
                    });
                });
            });
        }
    });
}
