//! System fonts with fallbacks, and the window icon taken from the bridge ICO.
use eframe::egui::FontFamily;
use neo_config::theme;

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-config-theme-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn system_fonts_come_first_and_fall_back_to_egui() {
    let windows = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("Fonts");
    let fonts = theme::fonts(&windows);
    assert_eq!(fonts.families[&theme::display()][..2], ["display".to_string(), "body".to_string()]);
    assert_eq!(fonts.families[&FontFamily::Proportional][0], "body");
    assert_eq!(fonts.families[&FontFamily::Monospace][0], "mono");
    // Fallback files: Segoe UI and Consolas when the variable and Cascadia fonts are missing.
    let older = temp("older");
    for file in ["segoeui.ttf", "consola.ttf"] {
        std::fs::copy(windows.join(file), older.join(file)).unwrap();
    }
    let fonts = theme::fonts(&older);
    assert_eq!(fonts.families[&theme::display()][0], "body");
    assert!(fonts.font_data.contains_key("mono") && !fonts.font_data.contains_key("display"));
    // None at all: egui's own fonts only.
    let none = theme::fonts(&temp("none"));
    let builtin = eframe::egui::FontDefinitions::default();
    assert_eq!(none.families[&theme::display()], builtin.families[&FontFamily::Proportional]);
    assert_eq!(none.families[&FontFamily::Monospace], builtin.families[&FontFamily::Monospace]);
}

#[test]
fn the_window_icon_is_the_largest_png_in_the_ico() {
    let icon = theme::window_icon(theme::ICON).unwrap();
    assert_eq!((icon.width, icon.height, icon.rgba.len()), (256, 256, 256 * 256 * 4));
    // Not an ICO, a truncated one, an entry pointing past the end, an entry that is not a PNG.
    assert!(theme::window_icon(b"").is_none());
    assert!(theme::window_icon(&theme::ICON[..20]).is_none());
    assert!(theme::window_icon(&[0, 0, 1, 0, 0, 0]).is_none());
    let mut header = theme::ICON[..22].to_vec();
    header[4] = 1;
    assert!(theme::window_icon(&header).is_none());
    assert!(theme::window_icon(&header[..16]).is_none());
    // A PNG cut off in its image data.
    let mut cut = header.clone();
    let first = &theme::ICON[118..118 + 100];
    cut[14..18].copy_from_slice(&100u32.to_le_bytes());
    cut[18..22].copy_from_slice(&22u32.to_le_bytes());
    cut.extend_from_slice(first);
    assert!(theme::window_icon(&cut).is_none());
    let mut bogus = header.clone();
    bogus.extend_from_slice(b"not a png at all");
    bogus[14..18].copy_from_slice(&16u32.to_le_bytes());
    bogus[18..22].copy_from_slice(&22u32.to_le_bytes());
    assert!(theme::window_icon(&bogus).is_none());
    // A PNG that is not 8-bit RGBA.
    let mut gray = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut gray, 1, 1);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.write_header().unwrap().write_image_data(&[0]).unwrap();
    }
    let mut grey_ico = header;
    grey_ico[14..18].copy_from_slice(&(gray.len() as u32).to_le_bytes());
    grey_ico[18..22].copy_from_slice(&22u32.to_le_bytes());
    grey_ico.extend_from_slice(&gray);
    assert!(theme::window_icon(&grey_ico).is_none());
}

#[test]
fn the_dark_design_holds_on_a_light_windows_theme() {
    let ctx = eframe::egui::Context::default();
    ctx.set_theme(eframe::egui::Theme::Light);
    theme::install(&ctx, &temp("light"));
    for chosen in [eframe::egui::Theme::Light, eframe::egui::Theme::Dark] {
        ctx.set_theme(chosen);
        let style = ctx.global_style();
        assert!(style.text_styles.contains_key(&theme::h1()) && style.text_styles.contains_key(&theme::cap()));
        assert_eq!(style.visuals.panel_fill, theme::GROUND);
    }
}

#[test]
fn visuals_use_the_console_palette() {
    let visuals = theme::visuals();
    assert_eq!((visuals.panel_fill, visuals.selection.bg_fill), (theme::GROUND, theme::ACCENT));
    assert_eq!(theme::tint(theme::ACCENT, 0.5).a(), 128);
    assert_eq!(theme::over(theme::GROUND, theme::WHITE, 0.5), eframe::egui::Color32::from_rgb(133, 135, 138));
}
