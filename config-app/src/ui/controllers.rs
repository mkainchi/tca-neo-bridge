//! The Controllers page: which controllers take part (every Thrustmaster one automatically, or those ticked) and the
//! set they make, whose mapping the Mapping page edits.
use super::settings::columns;
use super::widgets::{self, cap, help, mono, panel, panel_head, toggle};
use crate::state::State;
use crate::theme;
use crate::worker::Request;
use eframe::egui::{self, RichText, Sense, Ui, Vec2};

pub fn page(ui: &mut Ui, state: &mut State) -> Vec<Request> {
    let mut requests = Vec::new();
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let cols = columns(full, &[1.0, 1.0], 12.0);
    let present = state.map.controllers.present.clone();
    let active = state.map.controllers.active.clone();
    let s = &mut state.settings;
    panel(ui, cols[0], |ui| {
        panel_head(
            ui,
            |ui| {
                ui.label(widgets::h3("Controllers taking part"));
            },
            |ui| {
                ui.label(cap("Hidden from the game while emulating"));
            },
        );
        ui.spacing_mut().item_spacing.y = 8.0;
        let automatic = s.draft.as_ref().is_some_and(|draft| draft.devices.is_empty());
        if toggle(ui, automatic, "Automatic: every Thrustmaster joystick that is connected", false, ui.available_width()) {
            requests.extend(s.set_automatic(!automatic, &active));
        }
        if present.is_empty() {
            ui.label(help("No controller is connected. Connect your HOTAS, then reopen Configure from the tray icon."));
        }
        for device in &present {
            if toggle(ui, s.takes_part(device), &format!("{} ({})", device.name, device.id), false, ui.available_width()) {
                requests.extend(s.toggle_device(&present, &device.id));
            }
        }
        ui.label(help("Untick a controller to leave it visible to the game; tick another vendor\u{2019}s controller to use it too. Changes apply when you save and start emulation again."));
    });
    let names: Vec<String> = state.map.controllers.active().iter().map(|device| device.name.clone()).collect();
    let saved = state.saved.as_ref().expect("the profile is loaded");
    let rows = [
        ("Controllers", if names.is_empty() { "None".to_string() } else { names.join(" + ") }),
        ("Set", if saved.setup.is_empty() { "\u{2014}".to_string() } else { saved.setup.clone() }),
        ("Mapping", if saved.setups.contains_key(&saved.setup) { "Saved for this set" } else { "Default for this set" }.to_string()),
    ];
    panel(ui, cols[1], |ui| {
        panel_head(
            ui,
            |ui| {
                ui.label(widgets::h3("This set"));
            },
            |_| {},
        );
        egui::Grid::new("this set").num_columns(2).spacing(Vec2::new(16.0, 10.0)).show(ui, |ui| {
            for (title, value) in rows {
                ui.label(cap(title));
                ui.label(RichText::new(value).font(mono(12.0)).color(theme::WHITE));
                ui.end_row();
            }
        });
        ui.add_space(8.0);
        ui.label(help("Reporting a new controller? Run neo-daemon.exe --probe and attach the probe.txt it opens."));
    });
    requests
}
