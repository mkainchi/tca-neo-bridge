//! The header's profile picker and its menu: switch to a saved profile, save the current settings under a name,
//! import or export a profile file, delete the active profile (twice-clicked), and print the cheat sheet.
use super::widgets::{self, cap, mono, Kind};
use crate::state::{State, Want};
use crate::theme;
use crate::worker::Request;
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Pos2, Rect, RichText, Role, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo};

pub const MENU_WIDTH: f32 = 320.0;

/// The picker: the active profile's name and a chevron; a click opens or closes the menu. Returns its rectangle.
pub fn picker(ui: &mut Ui, state: &mut State) -> Rect {
    let name = state.profile_name().to_string();
    let galley = ui.painter().layout_no_wrap(name.clone(), mono(12.0), theme::WHITE);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(galley.size().x + 40.0, 34.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, state.menu.open, format!("Profile: {name}")));
    let border = if state.menu.open || response.hovered() { theme::ACCENT } else { theme::LINE_2 };
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(3), theme::GROUND, Stroke::new(1.0, border), StrokeKind::Inside);
    painter.galley(Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, theme::WHITE);
    let chevron = Pos2::new(rect.right() - 16.0, rect.center().y);
    painter.line(vec![chevron + Vec2::new(-3.0, -1.5), chevron + Vec2::new(0.0, 1.5), chevron + Vec2::new(3.0, -1.5)], Stroke::new(1.6, theme::MUTED));
    widgets::focus_ring(ui, &response, rect);
    if response.clicked() {
        state.menu.open = !state.menu.open;
    }
    rect
}

/// One saved profile in the menu: its name, and "active" beside the active one. True when clicked.
fn profile_row(ui: &mut Ui, name: &str, active: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, active, name));
    let fill = if response.hovered() { theme::RAISE } else { Color32::TRANSPARENT };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), fill);
    painter.text(Pos2::new(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, name, mono(12.0), if active { theme::ACCENT } else { theme::INK });
    if active {
        painter.text(Pos2::new(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, "ACTIVE", mono(10.0), theme::OK_INK);
    }
    widgets::focus_ring(ui, &response, rect);
    response.clicked()
}

/// The menu under the picker; closes on Escape or a click outside it.
pub fn menu(ctx: &egui::Context, state: &mut State, picker: Rect) -> Vec<Request> {
    let mut requests = Vec::new();
    if !state.menu.open {
        return requests;
    }
    let area = egui::Area::new(egui::Id::NULL.with("profile menu")).order(egui::Order::Foreground).fixed_pos(Pos2::new(picker.right() - MENU_WIDTH, picker.bottom() + 6.0));
    let shown = area.show(ctx, |ui| {
        Frame::new().fill(theme::PANEL).stroke(Stroke::new(1.0, theme::LINE_2)).corner_radius(theme::RADIUS).inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.set_width(MENU_WIDTH - 24.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(cap("Switch profile"));
            let names = state.profiles.profiles.clone();
            if names.is_empty() {
                ui.label(widgets::help("No saved profiles yet. Save the current one under a name."));
            }
            for name in names {
                let active = state.profiles.active.as_deref() == Some(name.as_str());
                if profile_row(ui, &name, active) {
                    requests.extend(state.activate(&name));
                }
            }
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 84.0, 30.0), Sense::hover());
                ui.painter().rect_filled(rect, CornerRadius::same(3), theme::GROUND);
                let border = if state.menu.invalid { theme::HAZARD } else { theme::LINE_2 };
                let edit = egui::TextEdit::singleline(&mut state.menu.name).font(mono(12.0)).char_limit(40).hint_text("Name, e.g. Dogfight").frame(Frame::NONE).margin(Margin::ZERO);
                let field = ui.put(rect.shrink2(Vec2::new(8.0, 6.0)), edit);
                ui.ctx().accesskit_node_builder(field.id, |node| node.set_label("New profile name"));
                ui.painter().rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, if field.has_focus() { theme::ACCENT } else { border }), StrokeKind::Inside);
                let entered = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if widgets::button(ui, "Save as", "Save as", Kind::Primary, true) || entered {
                    requests.extend(state.save_as());
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if widgets::button(ui, "Import\u{2026}", "Import a profile file", Kind::Quiet, true) {
                    state.menu.want = Some(Want::Import);
                }
                if widgets::button(ui, "Export", "Export the saved settings", Kind::Quiet, true) {
                    state.menu.want = Some(Want::Export);
                }
                let active = state.profiles.active.clone();
                let text = match (&active, state.menu.confirm_delete) {
                    (Some(name), true) => format!("Delete {name}? Click again"),
                    _ => "Delete".to_string(),
                };
                if widgets::button(ui, &text, &text, Kind::Hazard, active.is_some()) {
                    requests.extend(state.delete());
                }
            });
            if let Some((text, error)) = &state.menu.note {
                ui.label(RichText::new(text).font(mono(12.0)).color(if *error { theme::DANGER_INK } else { theme::OK_INK }));
            }
            ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                ui.label(RichText::new(format!("Saved settings \u{b7} {}", state.profile_path)).font(mono(11.0)).color(theme::DIM));
            });
        });
    });
    let inside = shown.response.rect;
    let outside = ctx.input(|input| input.pointer.any_click() && input.pointer.interact_pos().is_some_and(|pos| !inside.contains(pos) && !picker.contains(pos)));
    if outside || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        state.menu.open = false;
    }
    requests
}

/// "Print cheat sheet": the saved profile as a printable page.
pub fn print(ui: &mut Ui, state: &mut State) {
    if widgets::button(ui, "Print cheat sheet", "Print cheat sheet", Kind::Quiet, state.saved.is_some()) {
        state.menu.want = Some(Want::Print);
    }
}
