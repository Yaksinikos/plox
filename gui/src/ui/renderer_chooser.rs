//! Startup dialog to pick the renderer. Always drawn on the OpenGL renderer.

use egui::{Color32, RichText};

use crate::{
    renderer::{self, Backend},
    user_config::RendererConfig,
};

/// State of the renderer dialog
#[derive(Debug, Clone)]
pub struct ChooserState {
    pub selected: Backend,
    pub dont_ask_again: bool,
    /// The previous wgpu session did not exit cleanly
    pub crashed: bool,
    /// Detected hardware wgpu adapter, if any
    pub wgpu_adapter: Option<String>,
}

impl ChooserState {
    #[must_use]
    pub const fn new(preselect: Backend, crashed: bool, wgpu_adapter: Option<String>) -> Self {
        Self {
            selected: preselect,
            dont_ask_again: false,
            crashed,
            wgpu_adapter,
        }
    }
}

/// Draws the dialog, returns the confirmed backend.
pub fn show(state: &mut ChooserState, ui: &mut egui::Ui) -> Option<Backend> {
    let mut confirmed = None;
    egui::CentralPanel::default().show(ui, |ui| {
        ui.heading("Choose renderer");
        ui.add_space(4.0);

        if state.crashed {
            ui.colored_label(
                Color32::ORANGE,
                "The wgpu renderer did not exit cleanly last time. OpenGL is recommended.",
            );
            ui.add_space(4.0);
        }

        ui.radio_value(
            &mut state.selected,
            Backend::Glow,
            "OpenGL (glow): works on legacy GPUs and drivers",
        );

        let wgpu_label = match (&state.wgpu_adapter, renderer::wgpu_compiled()) {
            (_, false) => "wgpu: not included in this build".to_owned(),
            (Some(adapter), true) => format!("wgpu (Vulkan/DX12/Metal): {adapter}"),
            (None, true) => "wgpu (Vulkan/DX12/Metal): no hardware adapter found".to_owned(),
        };
        ui.add_enabled_ui(renderer::wgpu_compiled(), |ui| {
            ui.radio_value(&mut state.selected, Backend::Wgpu, wgpu_label);
        });

        ui.add_space(8.0);
        ui.checkbox(&mut state.dont_ask_again, "Don't ask again");
        ui.label(
            RichText::new("Start with --choose-renderer to show this dialog again.")
                .small()
                .weak(),
        );

        ui.separator();
        if ui.button("Start").clicked() {
            RendererConfig {
                backend: state.selected.into(),
                ask_on_startup: !state.dont_ask_again,
            }
            .save();
            confirmed = Some(state.selected);
        }
    });
    confirmed
}
