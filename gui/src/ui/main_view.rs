use egui::{Color32, Label, Sense};
use log::{error, info};
use plox::{
    ESupportedGame, GRAPH_FILE,
    parser::Warning,
    pipeline::{ELoadStatus, PipelineResult},
    rules::EWarningRule,
    update_new_load_order,
};

use super::{EModListView, PloxApp};

pub fn menu_bar(app: &mut PloxApp, ui: &mut egui::Ui) {
    egui::Panel::top("top_panel").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open graph file").clicked() {
                    if let Err(e) = open::that(GRAPH_FILE) {
                        error!("Error opening file: {e}");
                    }
                    ui.close();
                }

                ui.separator();
                if ui.button("Settings").clicked() {
                    app.settings_open = true;
                    ui.close();
                }

                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("About", |ui| {
                ui.hyperlink_to("PLOX on GitHub", "https://github.com/rfuzzo/plox");
            });

            ui.add_space(16.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
    });
}

pub fn loading(app: &PloxApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.label("Loading...");
        ui.separator();
        ui.label(&app.async_log);
    });
}

pub fn show(app: &mut PloxApp, ui: &mut egui::Ui) {
    // Temporarily take the data so the view state can be mutated alongside it.
    let Some(data) = app.app_data.take() else {
        return;
    };
    let dark_mode = ui.style().visuals.dark_mode;

    egui::Panel::left("side_panel")
        .min_size(200_f32)
        .show(ui, |ui| side_panel(app, ui, &data, dark_mode));

    egui::CentralPanel::default().show(ui, |ui| warnings_panel(app, ui, &data, dark_mode));

    app.app_data = Some(data);
}

fn side_panel(app: &mut PloxApp, ui: &mut egui::Ui, data: &PipelineResult, dark_mode: bool) {
    ui.heading("Load Order");
    ui.horizontal(|ui| {
        ui.radio_value(&mut app.mod_list_view, EModListView::LoadOrder, "Old");
        ui.radio_value(&mut app.mod_list_view, EModListView::NewOrder, "New");
    });
    ui.separator();
    ui.add_space(4_f32);

    status_actions(app, ui, data);

    ui.separator();
    ui.add_space(4_f32);

    let order = match app.mod_list_view {
        EModListView::NewOrder => &data.new_order,
        EModListView::LoadOrder => &data.old_order,
    };
    egui::ScrollArea::vertical().show(ui, |ui| {
        for mod_name in order {
            mod_item(app, ui, data, mod_name, dark_mode);
        }
    });
}

fn status_actions(app: &mut PloxApp, ui: &mut egui::Ui, data: &PipelineResult) {
    match data.status {
        ELoadStatus::Conflicts => {
            ui.colored_label(
                Color32::RED,
                "Mods were not sorted, please resolve conflicts first.",
            );
            if ui.button("Sort and ignore conflicts").clicked() {
                app.start_pipeline(true);
            }
        }
        ELoadStatus::Cycle => {
            ui.colored_label(
                Color32::RED,
                "Cycle detected in rules, please contact the rules maintainers.",
            );
        }
        ELoadStatus::Success => {
            let enabled = data.old_order != data.new_order;
            ui.add_enabled_ui(enabled, |ui| {
                let r = ui.add_sized([ui.available_width(), 0_f32], egui::Button::new("Accept"));
                if r.clicked() {
                    match update_new_load_order(
                        data.game,
                        &data.root,
                        &data.new_order,
                        app.settings.backup_load_order,
                    ) {
                        Ok(()) => info!("Update successful"),
                        Err(e) => error!("Could not update load order: {e}"),
                    }
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                r.on_disabled_hover_text("Mods are in correct order. No need to apply.");
            });
        }
    }
}

fn mod_item(
    app: &mut PloxApp,
    ui: &mut egui::Ui,
    data: &PipelineResult,
    mod_name: &String,
    dark_mode: bool,
) {
    let lower = mod_name.to_lowercase();
    let first_note = data
        .plugin_warning_map
        .iter()
        .find(|(p, _)| p.to_lowercase() == lower)
        .map(|(_, i)| &data.warnings[*i].rule);

    let selected = !app.plugin_filter.is_empty() && lower == app.plugin_filter.to_lowercase();
    let hovered_rule = first_note.filter(|_| app.plugin_hover_filter.contains(&lower));
    let bg_color = match hovered_rule {
        Some(rule) => get_color_for_rule(rule).to_opaque(),
        None if selected && dark_mode => Color32::DARK_BLUE,
        None if selected => Color32::LIGHT_BLUE,
        None => first_note.map_or(Color32::TRANSPARENT, |rule| {
            get_color_for_rule(rule).gamma_multiply(0.5)
        }),
    };

    egui::Frame::new().fill(bg_color).show(ui, |ui| {
        let label = if hovered_rule.is_some() && dark_mode {
            Label::new(egui::RichText::new(mod_name).color(Color32::BLACK))
        } else {
            Label::new(mod_name)
        }
        .sense(Sense::click());

        if ui.add_sized([ui.available_width(), 0_f32], label).clicked() {
            if app.plugin_filter == *mod_name {
                app.plugin_filter.clear();
            } else {
                app.plugin_filter.clone_from(mod_name);
            }
        }
    });
}

fn warnings_panel(app: &mut PloxApp, ui: &mut egui::Ui, data: &PipelineResult, dark_mode: bool) {
    let game_version = data.game_version.as_deref().unwrap_or("Unknown");
    if data.game == ESupportedGame::Cyberpunk {
        ui.heading(format!(
            "PLOX v{} - {:?} v{game_version}",
            crate::CARGO_PKG_VERSION,
            data.game
        ));
    } else {
        ui.heading(format!(
            "PLOX v{} - {:?}",
            crate::CARGO_PKG_VERSION,
            data.game
        ));
    }

    ui.horizontal(|ui| {
        ui.toggle_value(&mut app.show_notes, "Notes");
        ui.toggle_value(&mut app.show_conflicts, "Conflicts");
        ui.toggle_value(&mut app.show_requires, "Requires");
        ui.toggle_value(&mut app.show_patches, "Patches");
        ui.separator();
        ui.add(egui::TextEdit::singleline(&mut app.text_filter).hint_text("Filter"));
    });

    egui::ScrollArea::vertical().show(ui, |ui| {
        app.plugin_hover_filter.clear();
        for (i, w) in data.warnings.iter().enumerate() {
            if is_visible(app, w) {
                warning_item(app, ui, i, w, dark_mode);
            }
        }
    });
}

fn is_visible(app: &PloxApp, w: &Warning) -> bool {
    let kind_shown = match w.rule {
        EWarningRule::Note(_) => app.show_notes,
        EWarningRule::Conflict(_) => app.show_conflicts,
        EWarningRule::Requires(_) => app.show_requires,
        EWarningRule::Patch(_) => app.show_patches,
    };
    if !kind_shown {
        return false;
    }

    if !app.text_filter.is_empty() {
        let filter = app.text_filter.to_lowercase();
        if !w.get_rule_name().to_lowercase().contains(&filter)
            && !w.get_ui_comment().to_lowercase().contains(&filter)
        {
            return false;
        }
    }

    if !app.plugin_filter.is_empty() {
        let filter = app.plugin_filter.to_lowercase();
        return w.get_plugins().iter().any(|p| p.to_lowercase() == filter);
    }

    true
}

fn warning_item(app: &mut PloxApp, ui: &mut egui::Ui, i: usize, w: &Warning, dark_mode: bool) {
    let mut frame = egui::Frame::default().inner_margin(4.0).begin(ui);
    frame
        .content_ui
        .colored_label(get_color_for_rule(&w.rule), w.get_rule_name());
    frame.content_ui.label(w.get_ui_comment());
    frame.content_ui.push_id(i, |ui| {
        ui.collapsing("Plugins Affected", |ui| {
            for plugin in &w.get_plugins() {
                ui.label(plugin);
            }
        });
    });

    let response = frame.allocate_space(ui);
    if response.hovered() {
        frame.frame.fill = if dark_mode {
            Color32::DARK_GRAY
        } else {
            Color32::LIGHT_GRAY
        };
        app.plugin_hover_filter = w.get_plugins();
    }
    frame.paint(ui);
}

/// Highlighting according to <https://github.com/rfuzzo/mlox/blob/master/Documentation/Rule%20Guidelines.md#highlighting>
fn get_color_for_rule(rule: &EWarningRule) -> Color32 {
    match rule {
        EWarningRule::Note(note) => {
            if note.comment.starts_with("!!!") {
                Color32::RED
            } else if note.comment.starts_with("!!") {
                Color32::ORANGE
            } else if note.comment.starts_with('!') {
                Color32::LIGHT_BLUE
            } else {
                Color32::DARK_GREEN
            }
        }
        EWarningRule::Conflict(_) | EWarningRule::Patch(_) => Color32::YELLOW,
        EWarningRule::Requires(_) => Color32::RED,
    }
}
