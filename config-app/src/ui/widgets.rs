//! The design's controls, shared by the pages: bracketed panels, segmented choices, chips, buttons and labels.
use crate::theme::{self, tint};
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, Response, RichText, Role, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo};

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// An uppercase mono caption ("SIDESTICK", "BEHAVIOUR").
pub fn cap(text: &str) -> RichText {
    RichText::new(text.to_uppercase()).font(mono(11.0)).color(theme::DIM)
}

pub fn help(text: &str) -> RichText {
    RichText::new(text).size(12.0).color(theme::MUTED)
}

/// Accent L-shapes on a panel's top-left and bottom-right corners.
pub fn brackets(painter: &egui::Painter, rect: Rect) {
    let stroke = Stroke::new(2.0, theme::ACCENT);
    let (a, b) = (rect.left_top(), rect.right_bottom());
    painter.line(vec![a + Vec2::new(0.0, 7.0), a, a + Vec2::new(7.0, 0.0)], stroke);
    painter.line(vec![b - Vec2::new(0.0, 7.0), b, b - Vec2::new(7.0, 0.0)], stroke);
}

/// A panel filling `rect`: the design's raised surface with bracketed corners.
pub fn panel<R>(ui: &mut Ui, rect: Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.painter().rect(rect, CornerRadius::same(theme::RADIUS), theme::PANEL, Stroke::new(1.0, theme::LINE), StrokeKind::Inside);
    brackets(ui.painter(), rect.shrink(0.5));
    let inner = rect.shrink(12.0);
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)), |ui| {
        ui.set_clip_rect(rect);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
        add(ui)
    })
    .inner
}

/// A panel's title row with a rule beneath; `right` fills its right-hand side.
pub fn panel_head(ui: &mut Ui, title: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    let response = ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        title(ui);
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    let rect = response.response.rect;
    ui.painter().hline(rect.x_range(), rect.bottom() + 5.0, Stroke::new(1.0, theme::LINE));
    ui.add_space(2.0);
}

pub fn h3(text: &str) -> RichText {
    RichText::new(text.to_uppercase()).font(FontId::new(13.0, theme::display())).color(theme::WHITE).strong()
}

/// A segmented choice sized to its options; returns the index of the option clicked, if any. Options: (text,
/// accessible label, enabled). `amber` is the option shown in amber while chosen (the shift layer).
pub fn seg(ui: &mut Ui, options: &[(&str, &str, bool)], current: usize, amber: Option<usize>) -> Option<usize> {
    let widths: Vec<f32> = options.iter().map(|(text, _, _)| (ui.painter().layout_no_wrap(text.to_string(), mono(11.5), theme::MUTED).size().x + 20.0).max(32.0)).collect();
    let total = widths.iter().sum::<f32>() + 2.0 * (options.len() as f32 - 1.0) + 6.0;
    let (outer, _) = ui.allocate_exact_size(Vec2::new(total, 36.0), Sense::hover());
    ui.painter().rect(outer, CornerRadius::same(theme::RADIUS), theme::GROUND, Stroke::new(1.0, theme::LINE_2), StrokeKind::Inside);
    let mut chosen = None;
    let mut left = outer.left() + 3.0;
    for (value, (&(text, label, enabled), width)) in options.iter().zip(widths).enumerate() {
        let rect = Rect::from_min_size(Pos2::new(left, outer.top() + 3.0), Vec2::new(width, 30.0));
        left += width + 2.0;
        let on = value == current;
        let response = ui.interact(rect, ui.scope_id().with(("seg", label)), if enabled { Sense::click() } else { Sense::hover() });
        response.widget_info(|| WidgetInfo::selected(Role::RadioButton, enabled, on, label));
        let fill = match (on, amber == Some(value)) {
            (true, true) => theme::WARN,
            (true, false) => theme::ACCENT,
            _ => Color32::TRANSPARENT,
        };
        let ink = if on {
            theme::GROUND
        } else if !enabled {
            theme::MUTED.gamma_multiply(0.4)
        } else if response.hovered() {
            theme::INK
        } else {
            theme::MUTED
        };
        ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
        focus_ring(ui, &response, rect);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, mono(11.5), ink);
        if response.clicked() && !on {
            chosen = Some(value);
        }
    }
    chosen
}
/// The design's 2px accent outline, 2px out, while the keyboard focuses a control.
pub fn focus_ring(ui: &Ui, response: &Response, rect: Rect) {
    if response.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same(theme::RADIUS + 2), Stroke::new(2.0, theme::ACCENT), StrokeKind::Outside);
    }
}

/// How a chip shows: off, selected, feeding the inspected target, held down, or disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipState {
    Off,
    Selected,
    Feed,
    Held,
    Disabled,
}

pub const CHIP: Vec2 = Vec2::new(28.0, 28.0);

/// A small square choice ("9", "Up"): 28 points high, at least 28 wide.
pub fn chip(ui: &mut Ui, text: &str, label: &str, state: ChipState) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), mono(11.0), theme::SOFT);
    let size = Vec2::new((galley.size().x + 14.0).max(CHIP.x), CHIP.y);
    let enabled = state != ChipState::Disabled;
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, state == ChipState::Selected, label));
    let (fill, border, ink) = match state {
        ChipState::Selected => (theme::ACCENT, theme::ACCENT, theme::GROUND),
        ChipState::Feed => (tint(theme::ACCENT, 0.1), theme::ACCENT, theme::ACCENT),
        ChipState::Held => (tint(theme::OK, 0.15), theme::OK, theme::OK_INK),
        ChipState::Disabled => (tint(theme::RAISE, 0.4), tint(theme::LINE_2, 0.4), tint(theme::SOFT, 0.4)),
        ChipState::Off if response.hovered() => (theme::RAISE, theme::ACCENT, theme::SOFT),
        ChipState::Off => (theme::RAISE, theme::LINE_2, theme::SOFT),
    };
    ui.painter().rect(rect, CornerRadius::same(3), fill, Stroke::new(1.0, border), StrokeKind::Inside);
    focus_ring(ui, &response, rect);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, mono(11.0), ink);
    if enabled {
        response
    } else {
        response.on_hover_text(label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Plain,
    Quiet,
}

/// A small button (30 points high); disabled buttons fade and ignore clicks.
pub fn button(ui: &mut Ui, text: &str, label: &str, kind: Kind, enabled: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(12.0), theme::INK);
    let size = Vec2::new(galley.size().x + 20.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
    let hovered = enabled && response.hovered();
    let (fill, border, ink) = match kind {
        Kind::Primary if hovered => (theme::ACCENT_HOVER, theme::ACCENT_HOVER, theme::GROUND),
        Kind::Primary => (theme::ACCENT, theme::ACCENT, theme::GROUND),
        Kind::Plain => (theme::RAISE, if hovered { theme::ACCENT } else { theme::LINE_2 }, theme::INK),
        Kind::Quiet => (Color32::TRANSPARENT, if hovered { theme::ACCENT } else { theme::LINE_2 }, theme::INK),
    };
    let fade = |color: Color32| if enabled { color } else { color.gamma_multiply(0.4) };
    ui.painter().rect(rect, CornerRadius::same(theme::RADIUS), fade(fill), Stroke::new(1.0, fade(border)), StrokeKind::Inside);
    focus_ring(ui, &response, rect);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, egui::FontId::proportional(12.0), fade(ink));
    response.clicked()
}

/// A dot followed by text in a bordered strip: the context line and the live well.
pub fn strip(ui: &mut Ui, text: &str, height: f32, (border, fill, ink, dot): (Color32, Color32, Color32, Color32)) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect(rect, CornerRadius::same(3), fill, Stroke::new(1.0, border), StrokeKind::Inside);
    painter.circle_filled(Pos2::new(rect.left() + 13.5, rect.center().y), 3.5, dot);
    painter.text(Pos2::new(rect.left() + 24.0, rect.center().y), Align2::LEFT_CENTER, text, mono(11.5), ink);
}
