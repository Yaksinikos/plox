//! wgpu backend, restricted to Vulkan, DX12 and Metal.

use eframe::{egui_wgpu::WgpuSetup, wgpu};

const BACKENDS: wgpu::Backends = wgpu::Backends::VULKAN
    .union(wgpu::Backends::DX12)
    .union(wgpu::Backends::METAL);

/// Returns a description of the first hardware adapter. Software adapters
/// (lavapipe, WARP) don't count, OpenGL is the better choice there.
pub fn probe() -> Option<String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    desc.backends = BACKENDS;
    let instance = wgpu::Instance::new(desc);
    let adapters = pollster::block_on(instance.enumerate_adapters(BACKENDS));
    adapters
        .iter()
        .map(wgpu::Adapter::get_info)
        .find(|info| info.device_type != wgpu::DeviceType::Cpu)
        .map(|info| format!("{} ({:?})", info.name, info.backend))
}

pub fn configure(options: &mut eframe::NativeOptions) {
    options.renderer = eframe::Renderer::Wgpu;
    options.multisampling = 0;
    options.depth_buffer = 0;
    if let WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = BACKENDS;
    }
}

pub fn info(frame: &eframe::Frame) -> Option<String> {
    let info = frame.wgpu_render_state()?.adapter.get_info();
    Some(format!("wgpu {:?} ({})", info.backend, info.name))
}
