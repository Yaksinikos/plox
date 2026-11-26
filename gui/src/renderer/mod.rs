//! Renderer backends. Only this module and `launcher` know which graphics API is used.
//!
//! - `glow`: OpenGL 2.1+ / GLES 2.0+, always compiled in, for legacy GPUs.
//! - `wgpu`: Vulkan / DX12 / Metal, behind the default-on `wgpu` feature.

mod glow;
#[cfg(feature = "wgpu")]
mod wgpu;

use serde::{Deserialize, Serialize};

/// Renderer requested by the user (CLI, env or config)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum RendererChoice {
    /// wgpu if a hardware adapter is available, glow otherwise
    #[default]
    Auto,
    /// OpenGL, works on legacy GPUs
    Glow,
    /// Vulkan / DX12 / Metal
    Wgpu,
}

impl RendererChoice {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        <Self as clap::ValueEnum>::from_str(s.trim(), true).ok()
    }
}

/// Renderer that is actually used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Glow,
    Wgpu,
}

impl Backend {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Glow => "glow",
            Self::Wgpu => "wgpu",
        }
    }
}

impl From<Backend> for RendererChoice {
    fn from(backend: Backend) -> Self {
        match backend {
            Backend::Glow => Self::Glow,
            Backend::Wgpu => Self::Wgpu,
        }
    }
}

/// Whether this build contains the wgpu renderer
#[must_use]
pub const fn wgpu_compiled() -> bool {
    cfg!(feature = "wgpu")
}

/// Description of the hardware wgpu adapter, `None` if there is none (or only a software one)
#[must_use]
#[cfg_attr(
    not(feature = "wgpu"),
    expect(
        clippy::missing_const_for_fn,
        reason = "only const without the wgpu feature"
    )
)]
pub fn probe_wgpu() -> Option<String> {
    #[cfg(feature = "wgpu")]
    {
        wgpu::probe()
    }
    #[cfg(not(feature = "wgpu"))]
    {
        None
    }
}

/// Resolves the user's choice to a backend available in this build and on this machine.
#[must_use]
pub fn resolve(choice: RendererChoice) -> Backend {
    match choice {
        RendererChoice::Glow => Backend::Glow,
        RendererChoice::Wgpu if wgpu_compiled() => Backend::Wgpu,
        RendererChoice::Wgpu => {
            log::warn!("This build has no wgpu renderer, using OpenGL");
            Backend::Glow
        }
        RendererChoice::Auto => probe_wgpu().map_or_else(
            || {
                log::info!("No hardware wgpu adapter found, using OpenGL");
                Backend::Glow
            },
            |adapter| {
                log::info!("Hardware wgpu adapter found: {adapter}");
                Backend::Wgpu
            },
        ),
    }
}

/// Backend specific `NativeOptions` adjustments
pub const fn configure(options: &mut eframe::NativeOptions, backend: Backend) {
    match backend {
        Backend::Glow => glow::configure(options),
        #[cfg(feature = "wgpu")]
        Backend::Wgpu => wgpu::configure(options),
        #[cfg(not(feature = "wgpu"))]
        Backend::Wgpu => glow::configure(options),
    }
}

/// Human readable info about the active renderer, e.g. for the status bar
#[must_use]
pub fn active_info(frame: &eframe::Frame) -> String {
    #[cfg(feature = "wgpu")]
    if let Some(info) = wgpu::info(frame) {
        return info;
    }
    glow::info(frame).unwrap_or_else(|| "Unknown renderer".to_owned())
}
