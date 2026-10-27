//! The one-screen shell: header with status pills, launch warnings, the rail, the page, and the save bar.
pub mod mapping;
pub mod settings;
pub mod widgets;

use crate::settings::Tone as SettingsTone;
use crate::state::{pills, Page, State, Tone};
use crate::worker::Request;
use crate::theme::{self, tint};
use eframe::egui::{self, epaint::CubicBezierShape, Align, Color32, CornerRadius, Frame, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, Role, WidgetInfo};

/// Windows narrower than this get an icon rail and fewer header pills.
pub const COMPACT_BELOW: f32 = 1500.0;
pub const RAIL: f32 = 196.0;
pub const RAIL_COMPACT: f32 = 64.0;

/// Line art in a square viewBox: polylines, cubic curves, circles and rectangles.
pub enum Mark {
    Line(&'static [(f32, f32)]),
    Cubic([(f32, f32); 4]),
    Circle((f32, f32), f32),
    Rect((f32, f32), (f32, f32)),
}

/// The rail icons (20 × 20), drawn from the former page's SVG paths.
pub fn icon(page: Page) -> &'static [Mark] {
    use Mark::*;
    match page {
        Page::Mapping => &[Rect((3.0, 4.0), (4.0, 4.0)), Rect((3.0, 12.0), (4.0, 4.0)), Line(&[(9.0, 4.0), (17.0, 4.0)]), Line(&[(9.0, 7.0), (14.0, 7.0)]), Line(&[(9.0, 12.0), (17.0, 12.0)]), Line(&[(9.0, 15.0), (14.0, 15.0)])],
        Page::Axes => &[Line(&[(10.0, 2.0), (10.0, 18.0)]), Line(&[(2.0, 10.0), (18.0, 10.0)]), Cubic([(5.0, 15.0), (8.0, 15.0), (9.0, 12.0), (10.0, 10.0)]), Cubic([(10.0, 10.0), (11.0, 8.0), (12.0, 5.0), (15.0, 5.0)])],
        Page::Throttle => &[Line(&[(7.0, 17.0), (7.0, 9.0), (10.0, 3.0), (13.0, 9.0), (13.0, 17.0)]), Line(&[(5.0, 17.0), (15.0, 17.0)]), Line(&[(8.5, 11.0), (11.5, 11.0)])],
        Page::Camera => &[
            Cubic([(2.0, 10.0), (2.0, 10.0), (5.0, 5.0), (10.0, 5.0)]),
            Cubic([(10.0, 5.0), (15.0, 5.0), (18.0, 10.0), (18.0, 10.0)]),
            Cubic([(18.0, 10.0), (18.0, 10.0), (15.0, 15.0), (10.0, 15.0)]),
            Cubic([(10.0, 15.0), (5.0, 15.0), (2.0, 10.0), (2.0, 10.0)]),
            Circle((10.0, 10.0), 2.5),
        ],
        Page::Helpers => &[Line(&[(4.0, 5.0), (16.0, 5.0)]), Line(&[(4.0, 10.0), (16.0, 10.0)]), Line(&[(4.0, 15.0), (16.0, 15.0)]), Line(&[(7.0, 3.0), (7.0, 7.0)]), Line(&[(13.0, 8.0), (13.0, 12.0)]), Line(&[(9.0, 13.0), (9.0, 17.0)])],
        Page::Live => &[Line(&[(2.0, 10.0), (5.0, 10.0), (7.0, 5.0), (10.0, 15.0), (12.0, 9.0), (14.0, 12.0), (18.0, 12.0)])],
    }
}

/// The bridge mark beside the title (24 × 24).
pub const BRAND: &[Mark] = &[
    Mark::Circle((5.0, 12.0), 2.5),
    Mark::Circle((19.0, 12.0), 2.5),
    Mark::Line(&[(7.5, 12.0), (16.5, 12.0)]),
    Mark::Line(&[(5.0, 9.5), (5.0, 5.0), (19.0, 5.0), (19.0, 9.5)]),
    Mark::Line(&[(5.0, 14.5), (5.0, 19.0), (19.0, 19.0), (19.0, 14.5)]),
];

/// The warning sign before each launch warning (20 × 20).
pub const WARNING: &[Mark] = &[Mark::Line(&[(10.0, 2.5), (18.5, 17.0), (1.5, 17.0), (10.0, 2.5)]), Mark::Line(&[(10.0, 7.5), (10.0, 11.5)]), Mark::Circle((10.0, 14.2), 0.4)];

/// Draw `marks` from a `view`-unit square into `rect`.
pub fn draw(painter: &egui::Painter, rect: Rect, view: f32, marks: &[Mark], stroke: Stroke) {
    let scale = rect.width() / view;
    let at = |(x, y): (f32, f32)| rect.min + Vec2::new(x, y) * scale;
    for mark in marks {
        match mark {
            Mark::Line(points) => {
                painter.line(points.iter().copied().map(at).collect(), stroke);
            }
            Mark::Cubic(points) => {
                painter.add(CubicBezierShape::from_points_stroke(points.map(at), false, Color32::TRANSPARENT, stroke));
            }
            Mark::Circle(centre, radius) => {
                painter.circle_stroke(at(*centre), radius * scale, stroke);
            }
            Mark::Rect(corner, size) => {
                painter.rect_stroke(Rect::from_min_size(at(*corner), Vec2::new(size.0, size.1) * scale), 0.0, stroke, egui::StrokeKind::Middle);
            }
        }
    }
}

fn tone_colours(tone: Tone) -> (Color32, Color32, Color32, Color32) {
    // (border, fill, text, dot)
    match tone {
        Tone::Off => (theme::LINE_2, theme::RAISE, theme::SOFT, theme::LINE_2),
        Tone::Ok => (theme::LINE_2, theme::RAISE, theme::SOFT, theme::OK),
        Tone::Busy => (theme::LINE_2, theme::RAISE, theme::SOFT, theme::WARN),
        Tone::Fault => (tint(theme::HAZARD, 0.5), tint(theme::HAZARD, 0.1), theme::DANGER_INK, theme::HAZARD),
    }
}

/// A status pill: a coloured dot and a short mono label, sized to its text.
pub fn pill(ui: &mut Ui, text: &str, colours: (Color32, Color32, Color32, Color32)) {
    let (border, fill, ink, dot) = colours;
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::monospace(11.0), ink);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(galley.size().x + 34.0, 28.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(3), fill, Stroke::new(1.0, border), egui::StrokeKind::Inside);
    painter.circle_filled(Pos2::new(rect.left() + 13.5, rect.center().y), 3.5, dot);
    painter.galley(Pos2::new(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0), galley, ink);
}

/// The daemon's own pill is tinted while it emulates or changes state.
fn status_colours(tone: Tone) -> (Color32, Color32, Color32, Color32) {
    match tone {
        Tone::Ok => (tint(theme::OK, 0.5), tint(theme::OK, 0.1), theme::OK_INK, theme::OK),
        Tone::Busy => (tint(theme::WARN, 0.5), theme::RAISE, theme::WARN_INK, theme::WARN),
        other => tone_colours(other),
    }
}

/// HidHide's dot is cyan while it hides the TCA.
fn hidhide_colours(tone: Tone) -> (Color32, Color32, Color32, Color32) {
    let (border, fill, ink, dot) = tone_colours(tone);
    (border, fill, ink, if tone == Tone::Ok { theme::ACCENT } else { dot })
}

fn preview_colours() -> (Color32, Color32, Color32, Color32) {
    (tint(theme::WARN, 0.6), tint(theme::WARN, 0.14), theme::WARN_INK, theme::WARN)
}

fn header(ui: &mut Ui, state: &State, compact: bool) {
    let frame = Frame::new().fill(theme::PANEL).inner_margin(Margin::symmetric(16, 0));
    egui::Panel::top("header").frame(frame).exact_size(56.0).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
            ui.painter().rect(rect, CornerRadius::same(theme::RADIUS), tint(theme::ACCENT, 0.08), Stroke::new(1.0, tint(theme::ACCENT, 0.5)), egui::StrokeKind::Inside);
            draw(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(20.0)), 24.0, BRAND, Stroke::new(1.8, theme::ACCENT));
            ui.label(RichText::new("TCA-NEO BRIDGE").text_style(theme::h1()).color(theme::WHITE).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                // The profile picker opens with the profile actions in a later update; for now it names the profile.
                Frame::new().fill(theme::GROUND).stroke(Stroke::new(1.0, theme::LINE_2)).corner_radius(3).inner_margin(Margin::symmetric(10, 0)).show(ui, |ui| {
                    ui.set_height(34.0);
                    ui.horizontal_centered(|ui| ui.label(RichText::new(state.profile_name()).monospace().color(theme::WHITE).strong()));
                });
                ui.add_space(8.0);
                let shown = pills(&state.status, state.links.as_ref());
                pill(ui, shown.neo.1, tone_colours(shown.neo.0));
                if !compact {
                    pill(ui, shown.hidhide.1, hidhide_colours(shown.hidhide.0));
                    pill(ui, shown.tca.1, tone_colours(shown.tca.0));
                }
                pill(ui, &format!("Daemon: {}", state.status), status_colours(shown.status));
                if state.paused {
                    pill(ui, "Input paused", preview_colours());
                }
                if state.previewing() {
                    pill(ui, "Previewing unsaved", preview_colours());
                }
            });
        });
    });
}

fn warnings(ui: &mut Ui, state: &mut State) {
    if !state.show_warnings() {
        return;
    }
    let frame = Frame::new().fill(theme::over(theme::GROUND, theme::WARN, 0.08)).inner_margin(Margin::symmetric(16, 6));
    let banner = egui::Panel::top("warnings").frame(frame).show_separator_line(false).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for warning in &state.warnings {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                draw(ui.painter(), rect, 20.0, WARNING, Stroke::new(1.7, theme::WARN));
                ui.label(RichText::new(warning).small().color(theme::SOFT));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let dismiss = ui.add(egui::Button::new(RichText::new("Dismiss").small().color(theme::MUTED)).frame(false));
                dismiss.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Dismiss warnings"));
                if dismiss.clicked() {
                    state.dismissed = state.warnings.clone();
                }
            });
        });
    });
    let rect = banner.response.rect;
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, tint(theme::WARN, 0.35)));
}

/// One rail entry: icon and (unless compact) its uppercase label; the current page is accented.
fn rail_item(ui: &mut Ui, page: Page, current: bool, compact: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, current, page.title()));
    let colour = if current { theme::ACCENT } else if response.hovered() { theme::INK } else { theme::MUTED };
    let (fill, border) = if current {
        (tint(theme::ACCENT, 0.08), tint(theme::ACCENT, 0.4))
    } else if response.hovered() {
        (theme::RAISE, Color32::TRANSPARENT)
    } else {
        (Color32::TRANSPARENT, Color32::TRANSPARENT)
    };
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(theme::RADIUS), fill, Stroke::new(1.0, border), egui::StrokeKind::Inside);
    if response.has_focus() {
        // Keyboard focus: the design's 2px accent outline, 2px out.
        painter.rect_stroke(rect.expand(2.0), CornerRadius::same(theme::RADIUS + 2), Stroke::new(2.0, theme::ACCENT), egui::StrokeKind::Outside);
    }
    let icon_at = if compact { rect.center() - Vec2::splat(9.0) } else { Pos2::new(rect.left() + 12.0, rect.center().y - 9.0) };
    draw(painter, Rect::from_min_size(icon_at, Vec2::splat(18.0)), 20.0, icon(page), Stroke::new(1.7, colour));
    if !compact {
        let font = egui::FontId::monospace(11.5);
        painter.text(Pos2::new(rect.left() + 40.0, rect.center().y), egui::Align2::LEFT_CENTER, page.title().to_uppercase(), font, colour);
    }
    response.on_hover_text(page.title()).clicked()
}

fn rail(ui: &mut Ui, state: &mut State, compact: bool) -> Vec<Request> {
    let mut requests = Vec::new();
    let frame = Frame::new().fill(theme::BAR).inner_margin(Margin::symmetric(10, 12));
    egui::Panel::left("rail").frame(frame).resizable(false).exact_size(if compact { RAIL_COMPACT } else { RAIL }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for page in Page::ALL {
            if rail_item(ui, page, state.page == page, compact) {
                requests.extend(state.show(page));
            }
            if matches!(page, Page::Mapping | Page::Helpers) {
                ui.add_space(6.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().hline(rect.x_range().shrink(4.0), rect.center().y, Stroke::new(1.0, theme::LINE));
                ui.add_space(6.0);
            }
        }
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            let live = state.live();
            let (border, ink, dot) = if live { (tint(theme::OK, 0.45), theme::OK_INK, theme::OK) } else { (theme::LINE_2, theme::MUTED, theme::LINE_2) };
            Frame::new().stroke(Stroke::new(1.0, border)).corner_radius(3).inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 3.5, dot);
                    let text = if live { "LIVE INPUT" } else { "IDLE" };
                    if !compact {
                        ui.label(RichText::new(text).text_style(theme::cap()).color(ink).strong());
                    }
                })
                .response
                .widget_info(|| WidgetInfo::labeled(Role::Label, true, if live { "Live input" } else { "Idle" }));
            });
        });
    });
    requests
}

/// The save bar: what happened to the draft on the left; on the settings pages, Try in game, Discard, Reset to
/// defaults and Save settings on the right.
fn footer(ui: &mut Ui, state: &mut State) -> Vec<Request> {
    let mut requests = Vec::new();
    let frame = Frame::new().fill(theme::PANEL).inner_margin(Margin::symmetric(16, 0));
    egui::Panel::bottom("save-bar").frame(frame).exact_size(56.0).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            let message = state.settings.message.clone().map(|(text, tone)| (text, match tone {
                SettingsTone::Saved => theme::OK_INK,
                SettingsTone::Dirty => theme::WARN_INK,
                SettingsTone::Error => theme::DANGER_INK,
            }));
            if let Some((text, colour)) = message.or_else(|| state.note.clone().map(|note| (note, theme::DANGER_INK))) {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.5, colour);
                ui.label(RichText::new(text).size(12.0).family(egui::FontFamily::Monospace).color(colour));
            }
            // The actions edit the draft: settings pages only, once the profile (and with it the defaults) is in.
            let (true, Some(saved), Some(defaults)) = (state.page.settings() && state.settings.draft.is_some(), state.saved.clone(), state.defaults.clone()) else {
                return;
            };
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let s = &mut state.settings;
                if widgets::button(ui, "Save settings", "Save settings", widgets::Kind::Primary, !s.saving) {
                    requests.extend(s.save());
                }
                if widgets::button(ui, "Reset to defaults", "Reset to defaults", widgets::Kind::Hazard, true) {
                    requests.extend(s.reset(&defaults));
                }
                if widgets::button(ui, "Discard", "Discard", widgets::Kind::Plain, true) {
                    requests.extend(s.discard(&saved));
                }
                if widgets::toggle(ui, s.previewing, "Try in game", true, 120.0) {
                    let on = !s.previewing;
                    requests.extend(s.set_preview(on));
                }
            });
        });
    });
    requests
}

/// The page head: its title, its help (hidden in narrow windows) and, on the right, the page's own switch.
fn page_head(ui: &mut Ui, state: &mut State, compact: bool) -> Vec<Request> {
    ui.horizontal(|ui| {
        ui.set_min_height(38.0);
        ui.spacing_mut().item_spacing.x = 14.0;
        ui.label(RichText::new(state.page.title().to_uppercase()).heading().color(theme::WHITE).strong());
        if !compact {
            ui.label(RichText::new(state.page.help()).small().color(theme::MUTED));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| match (state.page, state.saved.clone()) {
            (Page::Mapping, Some(saved)) => {
                mapping::layer_switch(ui, &saved, &mut state.map);
                Vec::new()
            }
            (page, _) if page.settings() => settings::head_switch(ui, state),
            _ => Vec::new(),
        })
        .inner
    })
    .inner
}

fn page(ui: &mut Ui, state: &mut State, compact: bool) -> Vec<Request> {
    let frame = Frame::new().fill(theme::GROUND).inner_margin(Margin { left: 16, right: 16, top: 14, bottom: 12 });
    egui::CentralPanel::default()
        .frame(frame)
        .show(ui, |ui| {
            let mut requests = page_head(ui, state, compact);
            ui.add_space(12.0);
            match (state.page, state.saved.clone()) {
                (Page::Mapping, Some(saved)) => requests.extend(mapping::page(ui, &saved, &mut state.map, compact)),
                (page, Some(_)) if page.settings() => requests.extend(settings::page(ui, state, compact)),
                (page, _) => {
                    let text = if page == Page::Live { "This page is not in the native app yet." } else { "Loading the profile\u{2026}" };
                    Frame::new().fill(theme::PANEL).stroke(Stroke::new(1.0, theme::LINE)).corner_radius(theme::RADIUS).inner_margin(Margin::same(16)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(text).color(theme::MUTED));
                    });
                }
            }
            requests
        })
        .inner
}

/// The whole window for one frame; returns the requests its controls made.
pub fn shell(ui: &mut Ui, state: &mut State) -> Vec<Request> {
    let compact = ui.ctx().content_rect().width() < COMPACT_BELOW;
    header(ui, state, compact);
    warnings(ui, state);
    let mut requests = footer(ui, state);
    requests.extend(rail(ui, state, compact));
    requests.extend(page(ui, state, compact));
    requests
}