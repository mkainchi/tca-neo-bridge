//! The printable cheat sheet of the saved profile: a standalone A4 page (no script, no connection to the daemon)
//! that the browser opens and prints.
use crate::mapping::{current, mode_label, neo_action, short, sources, Layer, Mapping, Slot, HAT_SOURCES};
use neo_common::config::{Config, HatMode, Lever, Mode, ThrottleMode, YawSource};

/// Every value on the sheet goes through this.
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// "Gun, Missile, POV up": game actions where the NEO button has one.
fn action_name(mapping: &Mapping) -> String {
    let buttons = mapping.buttons.iter().map(|n| neo_action(*n).map_or(format!("NEO {n}"), String::from));
    let pov = mapping.pov.map(|direction| format!("POV {}", crate::mapping::direction_name(direction)));
    buttons.chain(pov).collect::<Vec<_>>().join(", ")
}

fn cell(text: &str) -> String {
    if text.is_empty() {
        "<td class=\"x\">\u{2014}</td>".into()
    } else {
        format!("<td>{}</td>", escape(text))
    }
}

fn table(pairs: &[(String, String)]) -> String {
    let rows: String = pairs.iter().map(|(key, value)| format!("<tr><td class=\"k\">{}</td><td>{}</td></tr>", escape(key), escape(value))).collect();
    format!("<table><tbody>{rows}</tbody></table>")
}

/// The sheet's body: buttons (press, long press, double tap, shift), axes and helpers.
pub fn body(config: &Config, profile: Option<&str>, date: &str) -> String {
    let hats = if config.hat == HatMode::Mapped { HAT_SOURCES.map(String::from).to_vec() } else { Vec::new() };
    let rows: String = sources()
        .into_iter()
        .chain(hats)
        .filter_map(|key| {
            let press = action_name(&current(config, &key, Slot::Press, Layer::Normal));
            let long = action_name(&current(config, &key, Slot::Long, Layer::Normal));
            let double = action_name(&current(config, &key, Slot::Double, Layer::Normal));
            let shifted = action_name(&current(config, &key, Slot::Press, Layer::Shift));
            if config.shift.as_deref() == Some(key.as_str()) || [&press, &long, &double, &shifted].iter().all(|text| text.is_empty()) {
                return None;
            }
            let tag = match config.behaviours.get(&key) {
                Some(mode) if *mode != Mode::Hold && !press.is_empty() => format!("<span class=\"tag\">{}</span>", escape(&mode_label(*mode).to_lowercase())),
                _ => String::new(),
            };
            let press_cell = if press.is_empty() { cell("") } else { format!("<td>{}{tag}</td>", escape(&press)) };
            let shift_cell = match (&config.shift, shifted.is_empty()) {
                (None, _) => String::new(),
                (Some(_), true) => "<td class=\"x\">same</td>".into(),
                (Some(_), false) => cell(&shifted),
            };
            Some(format!("<tr><td class=\"k\">{}</td>{press_cell}{}{}{shift_cell}</tr>", escape(&short(&key)), cell(&long), cell(&double)))
        })
        .collect();
    let curve = |tuning: &neo_common::config::AxisTuning| if tuning.curve == 0.0 { String::new() } else { format!(" \u{b7} curve {}%", (tuning.curve * 100.0).round()) };
    let lever = if config.throttle_lever == Lever::Right { "Right lever" } else { "Left lever" };
    let axes = vec![
        ("Stick".to_string(), format!("Roll / pitch{}", curve(&config.axes.roll))),
        (
            if config.yaw_source == YawSource::LeverDifferential { "Lever difference" } else { "Twist" }.to_string(),
            if config.yaw_source == YawSource::Off { "Yaw off".to_string() } else { format!("Yaw{}", curve(&config.axes.yaw)) },
        ),
        (
            if config.yaw_source == YawSource::LeverDifferential { "Both levers" } else { lever }.to_string(),
            if config.throttle.mode == ThrottleMode::Detents { "Throttle \u{b7} detents: idle brakes, CL cruises, boost from the last detent" } else { "Throttle" }.to_string(),
        ),
        (
            "Hat".to_string(),
            match (config.hat, config.camera.full_after_hold) {
                (HatMode::Mapped, _) => "Mapped as buttons (see above)".to_string(),
                (HatMode::Camera, true) => format!("Camera \u{b7} full after {} ms hold", config.timing.long_ms),
                (HatMode::Camera, false) => "Camera".to_string(),
            },
        ),
    ];
    let combo = |keys: &[String]| keys.iter().map(|key| short(key)).collect::<Vec<_>>().join(" + ");
    let helpers: Vec<(String, String)> = [
        config.precision.source.as_ref().map(|key| (format!("Hold {}", short(key)), format!("Precision aim \u{b7} {}%", (config.precision.scale * 100.0).round()))),
        config.shift.as_ref().map(|key| (format!("Hold {}", short(key)), "Shift layer".to_string())),
        (!config.combos.pause.is_empty()).then(|| (combo(&config.combos.pause), "Pause / resume input".to_string())),
        (!config.combos.next_profile.is_empty()).then(|| (combo(&config.combos.next_profile), "Next profile".to_string())),
        config.split_high_g.enabled.then(|| ("Levers split".to_string(), "High-G".to_string())),
    ]
    .into_iter()
    .flatten()
    .collect();
    let shift_head = config.shift.as_ref().map_or(String::new(), |key| format!("<th>Shift (hold {})</th>", escape(&short(key))));
    let helpers = if helpers.is_empty() { "<p>None set.</p>".to_string() } else { table(&helpers) };
    format!(
        "<div class=\"sheet-head\"><div><h1>TCA \u{2192} ACE COMBAT 8</h1><p>Profile: {}</p></div><p class=\"meta\">TCA-NEO bridge<br>Printed {}</p></div>\
         <h2>Buttons</h2><table><thead><tr><th>TCA control</th><th>Press</th><th>Long press</th><th>Double tap</th>{shift_head}</tr></thead><tbody>{rows}</tbody></table>\
         <div class=\"cols\"><div><h2>Axes</h2>{}</div><div><h2>Helpers</h2>{helpers}</div></div>\
         <p class=\"foot\">Long press: {} ms \u{b7} double tap: {} ms \u{b7} repeat every {} ms. Generated from the saved profile; reprint after changes.</p>",
        escape(profile.unwrap_or("Unsaved profile")),
        escape(date),
        table(&axes),
        config.timing.long_ms,
        config.timing.double_ms,
        config.timing.repeat_ms,
    )
}

const STYLE: &str = "@page{size:A4;margin:14mm}body{margin:0;color:#141821;background:#fff;font:11pt/1.4 \"Segoe UI Variable Text\",\"Segoe UI\",system-ui,sans-serif}\
h1{margin:0;font:700 20pt/1 Bahnschrift,\"Segoe UI\",sans-serif;letter-spacing:.04em;text-transform:uppercase}\
h2{margin:18px 0 6px;font:700 12pt Bahnschrift,\"Segoe UI\",sans-serif;letter-spacing:.06em;text-transform:uppercase}\
p{margin:0}.sheet-head{display:flex;justify-content:space-between;align-items:flex-end;padding-bottom:10px;border-bottom:2px solid #141821}\
.meta{font:9pt \"Cascadia Mono\",Consolas,monospace;color:#4a5568;text-align:right}table{width:100%;border-collapse:collapse}\
th{padding:5px 8px;border-bottom:1px solid #141821;text-align:left;font:700 8pt \"Cascadia Mono\",Consolas,monospace;letter-spacing:.06em;text-transform:uppercase;color:#4a5568}\
td{padding:5px 8px;border-bottom:1px solid #d5dbe3;vertical-align:top}td.k{font:600 10pt \"Cascadia Mono\",Consolas,monospace;white-space:nowrap}td.x{color:#6b7280}\
tr{break-inside:avoid}.tag{margin-left:6px;padding:0 5px;border:1px solid #8a94a3;border-radius:2px;font:8pt \"Cascadia Mono\",Consolas,monospace;color:#4a5568}\
.cols{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:24px}.foot{margin-top:18px;font-size:9pt;color:#4a5568}";

/// The whole page: printed as soon as the browser has laid it out.
pub fn page(config: &Config, profile: Option<&str>, date: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>TCA-NEO bridge cheat sheet</title><style>{STYLE}</style></head><body onload=\"print()\">{}</body></html>\n",
        body(config, profile, date)
    )
}

/// "27 Oct 2026", as the page printed it.
pub fn date(days_since_epoch: i64) -> String {
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{day} {} {year}", MONTHS[(month - 1) as usize])
}
