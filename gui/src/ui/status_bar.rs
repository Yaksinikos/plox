use super::PloxApp;

pub fn show(app: &PloxApp, ui: &mut egui::Ui) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(&app.last_message);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(info) = &app.renderer_info {
                    ui.weak(info);
                }
            });
        });
    });
}
