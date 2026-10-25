//! The approved look: a dark console with a cyan accent (the `:root` tokens of the former page's style.css), and the
//! Windows system fonts it named. Nothing is bundled; a missing font falls back to egui's own.
use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2};
use std::path::Path;
use std::sync::Arc;

pub const GROUND: Color32 = Color32::from_rgb(0x0b, 0x0f, 0x14);
pub const BAR: Color32 = Color32::from_rgb(0x0e, 0x13, 0x1a);
pub const PANEL: Color32 = Color32::from_rgb(0x12, 0x18, 0x20);
pub const RAISE: Color32 = Color32::from_rgb(0x1a, 0x22, 0x2d);
pub const LINE: Color32 = Color32::from_rgb(0x1e, 0x29, 0x3b);
pub const LINE_2: Color32 = Color32::from_rgb(0x2c, 0x3b, 0x4e);
pub const WHITE: Color32 = Color32::WHITE;
pub const INK: Color32 = Color32::from_rgb(0xdd, 0xe3, 0xee);
pub const SOFT: Color32 = Color32::from_rgb(0xc7, 0xd2, 0xde);
pub const MUTED: Color32 = Color32::from_rgb(0x94, 0xa3, 0xb8);
pub const DIM: Color32 = Color32::from_rgb(0x7b, 0x8b, 0xa3);
pub const ACCENT: Color32 = Color32::from_rgb(0x00, 0xd2, 0xff);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x38, 0xbd, 0xf8);
pub const OK: Color32 = Color32::from_rgb(0x10, 0xb9, 0x81);
pub const OK_INK: Color32 = Color32::from_rgb(0x6e, 0xe7, 0xb7);
pub const WARN: Color32 = Color32::from_rgb(0xf5, 0x9e, 0x0b);
pub const WARN_INK: Color32 = Color32::from_rgb(0xfc, 0xd3, 0x4d);
pub const HAZARD: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const DANGER_INK: Color32 = Color32::from_rgb(0xfc, 0xa5, 0xa5);
pub const RADIUS: u8 = 4;

/// `rgb(r g b / alpha)` from the stylesheet.
pub fn tint(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.0).round() as u8)
}

/// `color` at `alpha` laid over an opaque `base`: for fills with nothing painted beneath them.
pub fn over(base: Color32, color: Color32, alpha: f32) -> Color32 {
    let mix = |b: u8, c: u8| (f32::from(b) + (f32::from(c) - f32::from(b)) * alpha).round() as u8;
    Color32::from_rgb(mix(base.r(), color.r()), mix(base.g(), color.g()), mix(base.b(), color.b()))
}

/// Bahnschrift for headings, Segoe UI Variable for text, Cascadia Mono for figures and labels.
pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}

/// The window's fonts from `fonts` (normally `%WINDIR%\Fonts`): (family, first file found among the candidates).
pub const FACES: [(&str, &[&str]); 3] = [("display", &["bahnschrift.ttf"]), ("body", &["SegUIVar.ttf", "segoeui.ttf"]), ("mono", &["CascadiaMono.ttf", "consola.ttf"])];

pub fn fonts(dir: &Path) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let loaded: Vec<&str> = FACES
        .iter()
        .filter_map(|(name, files)| {
            let bytes = files.iter().find_map(|file| std::fs::read(dir.join(file)).ok())?;
            fonts.font_data.insert(name.to_string(), Arc::new(FontData::from_owned(bytes)));
            Some(*name)
        })
        .collect();
    // Each family tries its system faces first, then egui's built-in fonts.
    let chain = |preferred: &[&str], base: FontFamily| -> Vec<String> {
        preferred.iter().filter(|name| loaded.contains(name)).map(|name| name.to_string()).chain(fonts.families[&base].clone()).collect()
    };
    let families = [
        (display(), chain(&["display", "body"], FontFamily::Proportional)),
        (FontFamily::Proportional, chain(&["body"], FontFamily::Proportional)),
        (FontFamily::Monospace, chain(&["mono"], FontFamily::Monospace)),
    ];
    fonts.families.extend(families);
    fonts
}

/// The bridge mark, as the daemon's tray and Explorer show it.
pub const ICON: &[u8] = include_bytes!("../../daemon/assets/bridge.ico");

/// The largest image in an ICO of PNG images (as `scripts/build-icons.cjs` writes them), for the title bar and
/// taskbar; None if the file is not such an ICO.
pub fn window_icon(ico: &[u8]) -> Option<egui::IconData> {
    let word = |at: usize| ico.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let long = |at: usize| ico.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let entries = (0..word(4)?).map(|index| 6 + 16 * index);
    // A width byte of 0 means 256.
    let entry = entries.max_by_key(|entry| ico.get(*entry).map_or(0, |width| if *width == 0 { 256 } else { usize::from(*width) }))?;
    let (size, offset) = (long(entry + 8)?, long(entry + 12)?);
    let mut reader = png::Decoder::new(std::io::Cursor::new(ico.get(offset..offset + size)?)).read_info().ok()?;
    // An image too large to size fails to decode into the empty buffer.
    let mut rgba = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let frame = reader.next_frame(&mut rgba).ok()?;
    (frame.color_type == png::ColorType::Rgba && frame.bit_depth == png::BitDepth::Eight).then_some(egui::IconData { rgba, width: frame.width, height: frame.height })
}

/// Named text styles beyond egui's own.
pub fn h1() -> TextStyle {
    TextStyle::Name("h1".into())
}

pub fn cap() -> TextStyle {
    TextStyle::Name("cap".into())
}

/// The design is dark only, whatever Windows' own theme: both of egui's theme styles get the same look.
pub fn install(ctx: &egui::Context, font_dir: &Path) {
    ctx.set_fonts(fonts(font_dir));
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.visuals = visuals();
        style.text_styles = [
            (TextStyle::Body, FontId::proportional(13.0)),
            (TextStyle::Button, FontId::proportional(12.5)),
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Monospace, FontId::monospace(12.0)),
            (TextStyle::Heading, FontId::new(15.0, display())),
            (h1(), FontId::new(17.0, display())),
            (cap(), FontId::monospace(11.0)),
        ]
        .into();
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(14.0, 6.0);
        style.spacing.interact_size.y = 18.0;
        style.spacing.window_margin = Margin::same(16);
    });
}

pub fn visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = GROUND;
    visuals.window_fill = PANEL;
    visuals.window_stroke = Stroke::new(1.0, LINE);
    visuals.extreme_bg_color = GROUND;
    visuals.faint_bg_color = RAISE;
    visuals.code_bg_color = RAISE;
    visuals.hyperlink_color = ACCENT;
    visuals.override_text_color = None;
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, GROUND);
    let radius = CornerRadius::same(RADIUS);
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = PANEL;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    for (state, border, text) in [(&mut widgets.inactive, LINE_2, INK), (&mut widgets.hovered, ACCENT, WHITE), (&mut widgets.active, ACCENT, WHITE), (&mut widgets.open, ACCENT, WHITE)] {
        state.bg_fill = RAISE;
        state.weak_bg_fill = RAISE;
        state.bg_stroke = Stroke::new(1.0, border);
        state.fg_stroke = Stroke::new(1.0, text);
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    widgets.noninteractive.corner_radius = radius;
    visuals
}
