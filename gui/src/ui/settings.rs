use egui::{Id, Modal};

use crate::settings::{AppSettings, settings_file};

const LOG_LEVELS: [(&str, &str); 5] = [
    ("trace", "Trace"),
    ("debug", "Debug"),
    ("info", "Info"),
    ("warn", "Warn"),
    ("error", "Error"),
];

/// Shows the settings modal. Returns whether it should stay open.
pub fn show(ctx: &egui::Context, settings: &mut AppSettings) -> bool {
    let mut open = true;
    let modal = Modal::new(Id::unique("SettingsModal")).show(ctx, |ui| {
        ui.set_width(250.0);
        ui.heading("Settings");

        ui.checkbox(
            &mut settings.no_rules_download,
            "Do not download rules on startup",
        );

        ui.label("Log Level:");
        ui.horizontal(|ui| {
            for (value, label) in LOG_LEVELS {
                ui.radio_value(&mut settings.log_level, Some(value.to_owned()), label);
            }
        });

        ui.checkbox(&mut settings.log_to_file, "Log to file");
        ui.checkbox(&mut settings.ignore_warnings, "Ignore warnings");

        ui.separator();
        egui::Sides::new().show(
            ui,
            |_ui| {},
            |ui| {
                if ui.button("Save").clicked() {
                    settings.save(&settings_file());
                    open = false;
                }
                if ui.button("Cancel").clicked() {
                    open = false;
                }
            },
        );
    });

    open && !modal.should_close()
}
