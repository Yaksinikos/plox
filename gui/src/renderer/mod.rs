//! Renderer backends. Only this module and `launcher` know which graphics API is used.

mod glow;

/// Backend specific `NativeOptions` adjustments
pub fn configure(options: &mut eframe::NativeOptions) {
    glow::configure(options);
}

/// Human readable info about the active renderer, e.g. for the status bar
pub fn active_info(frame: &eframe::Frame) -> String {
    glow::info(frame).unwrap_or_else(|| "Unknown renderer".to_owned())
}
