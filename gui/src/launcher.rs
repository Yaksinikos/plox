//! Parses the command line, picks the renderer and starts the app.
//!
//! Renderer priority: `--renderer` → `PLOX_RENDERER` → user config → `auto`.
//! If wgpu fails to start, the process restarts itself on OpenGL.

use std::process::{Command, ExitCode};

use clap::Parser;
use log::{error, info, warn};

use crate::{
    PloxApp,
    renderer::{self, Backend, RendererChoice},
    ui::AppOptions,
    user_config::{RendererConfig, crash_marker},
};

const APP_TITLE: &str = "PLOX GUI";
const RENDERER_ENV: &str = "PLOX_RENDERER";
const FALLBACK_ENV: &str = "PLOX_RENDERER_FALLBACK";

#[derive(Debug, Clone, Parser)]
#[command(version, about = "PLOX - Plugin Load Order eXpert")]
pub struct Args {
    /// Renderer to use: auto, glow (OpenGL, legacy GPUs) or wgpu (Vulkan/DX12/Metal)
    #[arg(long, value_enum)]
    pub renderer: Option<RendererChoice>,

    /// Render a single frame and exit, to check that the renderer works
    #[arg(long)]
    pub smoke_test: bool,
}

/// Entry point of the GUI binary
pub fn main() -> ExitCode {
    let args = Args::parse();
    crate::ui::init_logger();
    let choice = requested_renderer(&args);
    let backend = renderer::resolve(choice);
    info!("Starting with renderer {}", backend.as_str());

    match run(backend, &args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if backend == Backend::Wgpu => {
            error!("wgpu renderer failed: {e}");
            restart_on_glow(&args)
        }
        Err(e) => {
            error!("OpenGL renderer failed: {e}");
            eprintln!(
                "PLOX could not start the OpenGL renderer: {e}\n\
                 PLOX needs OpenGL 2.1 or newer. Try updating the graphics driver, \
                 LIBGL_ALWAYS_SOFTWARE=1 on Linux, Mesa opengl32.dll on Windows, \
                 or use the plox command line tool."
            );
            ExitCode::FAILURE
        }
    }
}

fn requested_renderer(args: &Args) -> RendererChoice {
    if let Some(choice) = args.renderer {
        return choice;
    }
    if let Ok(value) = std::env::var(RENDERER_ENV) {
        match RendererChoice::parse(&value) {
            Some(choice) => return choice,
            None => warn!("Ignoring invalid {RENDERER_ENV}={value}"),
        }
    }
    if crash_marker::exists() {
        warn!("The wgpu renderer did not exit cleanly last time, using OpenGL");
        crash_marker::clear();
        return RendererChoice::Glow;
    }
    RendererConfig::load().backend
}

/// Restarts the current executable on OpenGL. Guarded against restart loops.
fn restart_on_glow(args: &Args) -> ExitCode {
    crash_marker::clear();
    if std::env::var_os(FALLBACK_ENV).is_some() {
        error!("Already running as renderer fallback, giving up");
        return ExitCode::FAILURE;
    }
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            error!("Cannot restart on OpenGL: {e}");
            return ExitCode::FAILURE;
        }
    };

    warn!("Restarting on the OpenGL renderer");
    let mut cmd = Command::new(exe);
    cmd.args(["--renderer", "glow"]).env(FALLBACK_ENV, "1");
    if args.smoke_test {
        cmd.arg("--smoke-test");
    }
    match cmd.status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            error!("OpenGL fallback exited with {status}");
            ExitCode::FAILURE
        }
        Err(e) => {
            error!("Cannot restart on OpenGL: {e}");
            ExitCode::FAILURE
        }
    }
}

fn native_options(backend: Backend) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([400.0, 300.0])
        .with_min_inner_size([300.0, 220.0])
        .with_app_id("plox");
    match eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => warn!("Could not load window icon: {e}"),
    }

    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    renderer::configure(&mut options, backend);
    options
}

/// Runs the GUI on `backend` until the window is closed.
fn run(backend: Backend, args: &Args) -> eframe::Result<()> {
    let app_options = AppOptions {
        smoke_test: args.smoke_test,
    };
    // A crash marker left behind means the wgpu session died (e.g. a driver crash).
    if backend == Backend::Wgpu {
        crash_marker::set();
    }
    let result = eframe::run_native(
        APP_TITLE,
        native_options(backend),
        Box::new(move |cc| Ok(Box::new(PloxApp::new(cc, app_options)))),
    );
    if backend == Backend::Wgpu && result.is_ok() {
        crash_marker::clear();
    }
    result
}
