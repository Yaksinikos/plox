//! OpenGL (glow) backend, works on OpenGL 2.1+ / GLES 2.0+ including legacy GPUs.

use eframe::glow::HasContext as _;

pub fn configure(options: &mut eframe::NativeOptions) {
    options.renderer = eframe::Renderer::Glow;
    // Keep requirements minimal for legacy drivers.
    options.multisampling = 0;
    options.depth_buffer = 0;
    options.stencil_buffer = 0;
    options.glow_options.vsync = true;
    options.glow_options.hardware_acceleration = eframe::egui_glow::HardwareAcceleration::Preferred;
}

pub fn info(frame: &eframe::Frame) -> Option<String> {
    let version = frame.gl()?.version();
    let kind = if version.is_embedded {
        "OpenGL ES"
    } else {
        "OpenGL"
    };
    Some(format!(
        "{kind} {}.{} {}",
        version.major, version.minor, version.vendor_info
    ))
}
