//! Builds the native window options and starts the app on the configured renderer.

use crate::{PloxApp, renderer};

const APP_TITLE: &str = "PLOX GUI";

fn native_options() -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([400.0, 300.0])
        .with_min_inner_size([300.0, 220.0])
        .with_app_id("plox");
    match eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => log::warn!("Could not load window icon: {e}"),
    }

    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    renderer::configure(&mut options);
    options
}

/// Runs the GUI until the window is closed.
///
/// # Errors
///
/// Returns an error if the window or the graphics context cannot be created.
pub fn run() -> eframe::Result<()> {
    eframe::run_native(
        APP_TITLE,
        native_options(),
        Box::new(|cc| Ok(Box::new(PloxApp::new(cc)))),
    )
}
