//! The Live page: every axis raw against what the NEO receives, the bridge's state, what is pressed, and the NEO's
//! buttons and POV as sent.
use super::settings::columns;
use super::widgets::{self, bar, mono, panel, panel_head};
use crate::mapping::{label, neo_action, short};
use crate::previews::{live_rows, live_states, pov, signed, Tone};
use crate::state::State;
use crate::theme::{self, tint};
use eframe::egui::{self, Align2, Color32, CornerRadius, Pos2, Rect, RichText, Role, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo};

const RAW: Color32 = Color32::from_rgba_premultiplied(89, 98, 110, 153);

fn tone_colours(tone: Tone) -> (Color32, Color32) {
    match tone {
        Tone::Ok => (tint(theme::OK, 0.6), theme::OK_INK),
        Tone::Warn => (tint(theme::WARN, 0.6), theme::WARN_INK),
        _ => (theme::LINE_2, theme::MUTED),
    }
}

/// A tile with centred uppercase text in its tone (the state row).
fn tile(ui: &mut Ui, rect: Rect, text: &str, tone: Tone) {
    let (border, ink) = tone_colours(tone);
    let response = ui.allocate_rect(rect, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    let painter = ui.painter();
    painter.rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, border), StrokeKind::Inside);
    let galley = painter.layout(text.to_uppercase(), mono(11.0), ink, rect.width() - 8.0);
    painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
}

pub fn page(ui: &mut Ui, state: &State) {
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let cols = columns(full, &[1.3, 1.0], 12.0);
    let saved = state.saved.as_ref().expect("the profile is loaded");
    let monitor = state.monitor.as_ref();
    panel(ui, cols[0], |ui| {
        panel_head(
            ui,
            |ui| {
                ui.label(widgets::h3("Axes \u{b7} raw \u{2192} NEO"));
            },
            |ui| {
                for (text, colour) in [("Sent to NEO", theme::ACCENT), ("TCA raw", RAW)] {
                    ui.label(RichText::new(text).size(11.5).color(theme::MUTED));
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 8.0), Sense::hover());
                    ui.painter().rect_filled(rect, CornerRadius::ZERO, colour);
                }
            },
        );
        let Some(monitor) = monitor else {
            let response = egui::Frame::new().inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(widgets::help("Start emulation to see the TCA and what the NEO receives."));
            });
            let r = response.response.rect;
            ui.painter().extend(egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(1.0, theme::LINE_2), 4.0, 3.0));
            return;
        };
        let rows = live_rows(monitor, &state.map.held, saved);
        let area = ui.available_rect_before_wrap();
        let step = area.height() / rows.len() as f32;
        for (index, (name, source, raw, sent)) in rows.into_iter().enumerate() {
            let row = Rect::from_min_size(Pos2::new(area.left(), area.top() + step * index as f32), Vec2::new(area.width(), step));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
            child.spacing_mut().item_spacing.x = 10.0;
            let (names, response) = child.allocate_exact_size(Vec2::new(120.0, 30.0), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, format!("{name} from {source}: {}", signed(sent))));
            child.painter().text(names.left_top() + Vec2::new(0.0, 2.0), Align2::LEFT_TOP, name, mono(11.5), theme::WHITE);
            child.painter().text(names.left_top() + Vec2::new(0.0, 17.0), Align2::LEFT_TOP, source, mono(11.0), theme::DIM);
            child.allocate_ui_with_layout(Vec2::new(child.available_width() - 70.0, 24.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                bar(ui, raw, 10.0, RAW, theme::GROUND);
                bar(ui, sent, 10.0, theme::ACCENT, theme::GROUND);
            });
            let (value, _) = child.allocate_exact_size(Vec2::new(60.0, 24.0), Sense::hover());
            child.painter().text(value.right_center(), Align2::RIGHT_CENTER, signed(sent), mono(11.5), theme::ACCENT);
        }
    });
    let right = cols[1];
    let state_panel = Rect::from_min_size(right.min, Vec2::new(right.width(), 130.0));
    let neo_panel = Rect::from_min_size(Pos2::new(right.left(), state_panel.bottom() + 12.0), Vec2::new(right.width(), 280.0));
    panel(ui, state_panel, |ui| {
        panel_head(
            ui,
            |ui| {
                ui.label(widgets::h3("State"));
            },
            |_| {},
        );
        let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
        let width = (row.width() - 20.0) / 5.0;
        for (index, (text, tone)) in live_states(monitor, state.paused, state.previewing()).into_iter().enumerate() {
            tile(ui, Rect::from_min_size(Pos2::new(row.left() + index as f32 * (width + 5.0), row.top()), Vec2::new(width, 32.0)), text, tone);
        }
        ui.horizontal(|ui| {
            let (caption, _) = ui.allocate_exact_size(Vec2::new(70.0, 28.0), Sense::hover());
            ui.painter().text(caption.left_center(), Align2::LEFT_CENTER, "PRESSED", mono(11.0), theme::DIM);
            if state.map.held.is_empty() {
                ui.label(RichText::new("Nothing pressed").size(11.5).color(theme::DIM));
            }
            for key in &state.map.held {
                let text = short(key);
                let galley = ui.painter().layout_no_wrap(text.clone(), mono(11.0), theme::OK_INK);
                let (rect, response) = ui.allocate_exact_size(Vec2::new((galley.size().x + 14.0).max(28.0), 28.0), Sense::hover());
                response.widget_info(|| WidgetInfo::labeled(Role::Label, true, label(key)));
                ui.painter().rect(rect, CornerRadius::same(3), tint(theme::OK, 0.15), Stroke::new(1.0, theme::OK), StrokeKind::Inside);
                ui.painter().galley(rect.center() - galley.size() / 2.0, galley, theme::OK_INK);
            }
        });
    });
    panel(ui, neo_panel, |ui| {
        panel_head(
            ui,
            |ui| {
                ui.label(widgets::h3("NEO buttons and POV"));
            },
            |_| {},
        );
        let buttons = monitor.map_or(0, |monitor| monitor.buttons);
        let (grid, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 3.0 * 43.0 + 10.0), Sense::hover());
        let width = (grid.width() - 20.0) / 5.0;
        for index in 0..15usize {
            let on = buttons & (1 << index) != 0;
            let cell = Rect::from_min_size(Pos2::new(grid.left() + (index % 5) as f32 * (width + 5.0), grid.top() + (index / 5) as f32 * 48.0), Vec2::new(width, 43.0));
            let action = neo_action(index as u8 + 1).unwrap_or("\u{2014}");
            let response = ui.allocate_rect(cell, Sense::hover());
            response.widget_info(|| WidgetInfo::selected(Role::Label, true, on, format!("NEO {} {action}", index + 1)));
            let (border, fill, number, ink) = if on { (theme::OK, tint(theme::OK, 0.15), theme::WHITE, theme::OK_INK) } else { (theme::LINE_2, theme::GROUND, theme::SOFT, theme::DIM) };
            let painter = ui.painter().with_clip_rect(cell);
            painter.rect(cell, CornerRadius::same(3), fill, Stroke::new(1.0, border), StrokeKind::Inside);
            painter.text(cell.left_top() + Vec2::new(6.0, 6.0), Align2::LEFT_TOP, (index + 1).to_string(), mono(12.0), number);
            painter.text(cell.left_top() + Vec2::new(6.0, 23.0), Align2::LEFT_TOP, action, mono(11.0), ink);
        }
        let (name, cell) = pov(monitor.map_or(8, |monitor| monitor.pov));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Image, true, &name));
            for index in 0..9usize {
                let square = Rect::from_min_size(rect.min + Vec2::new((index % 3) as f32 * 25.0, (index / 3) as f32 * 25.0), Vec2::splat(22.0));
                let on = cell == Some(index);
                ui.painter().rect(square, CornerRadius::same(3), if on { theme::OK } else { theme::GROUND }, Stroke::new(1.0, if on { theme::OK } else { theme::LINE_2 }), StrokeKind::Inside);
            }
            ui.label(widgets::help(&name));
        });
    });
}
