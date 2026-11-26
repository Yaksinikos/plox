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
    ui::{AppOptions, ChooserState},
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

    /// Show the renderer dialog even if "Don't ask again" was chosen
    #[arg(long, conflicts_with = "no_chooser")]
    pub choose_renderer: bool,

    /// Never show the renderer dialog
    #[arg(long)]
    pub no_chooser: bool,
}

/// How the app starts
enum Startup {
    /// Show the renderer dialog on OpenGL first
    Chooser(ChooserState),
    /// Start the main view directly
    Direct(Backend),
}

/// Entry point of the GUI binary
pub fn main() -> ExitCode {
    let args = Args::parse();
    crate::ui::init_logger();

    let (backend, chooser) = match startup(&args) {
        Startup::Chooser(state) => (Backend::Glow, Some(state)),
        Startup::Direct(backend) => (backend, None),
    };
    info!("Starting with renderer {}", backend.as_str());

    match run(backend, &args, chooser) {
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

/// Renderer forced by the command line or environment
fn forced_renderer(args: &Args) -> Option<RendererChoice> {
    if args.renderer.is_some() {
        return args.renderer;
    }
    let value = std::env::var(RENDERER_ENV).ok()?;
    let choice = RendererChoice::parse(&value);
    if choice.is_none() {
        warn!("Ignoring invalid {RENDERER_ENV}={value}");
    }
    choice
}

fn startup(args: &Args) -> Startup {
    let crashed = crash_marker::exists();
    if crashed {
        warn!("The wgpu renderer did not exit cleanly last time");
        crash_marker::clear();
    }
    let forced = forced_renderer(args);
    let config = RendererConfig::load();

    let show_chooser = !args.smoke_test
        && !args.no_chooser
        && (args.choose_renderer || (forced.is_none() && (crashed || config.ask_on_startup)));

    if !show_chooser {
        return match forced {
            Some(choice) => Startup::Direct(renderer::resolve(choice)),
            None if crashed => Startup::Direct(Backend::Glow),
            None => Startup::Direct(renderer::resolve(config.backend)),
        };
    }

    let wgpu_adapter = renderer::probe_wgpu();
    let preselect = if crashed {
        Backend::Glow
    } else {
        let wants_wgpu = match forced.unwrap_or(config.backend) {
            RendererChoice::Glow => false,
            RendererChoice::Wgpu => renderer::wgpu_compiled(),
            RendererChoice::Auto => wgpu_adapter.is_some(),
        };
        if wants_wgpu {
            Backend::Wgpu
        } else {
            Backend::Glow
        }
    };
    Startup::Chooser(ChooserState::new(preselect, crashed, wgpu_adapter))
}

/// Starts a new instance of this executable on `backend`, without the renderer dialog.
///
/// # Errors
///
/// Returns an error if the process cannot be spawned.
pub fn relaunch(backend: Backend) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    info!("Relaunching on renderer {}", backend.as_str());
    Command::new(exe)
        .args(["--renderer", backend.as_str(), "--no-chooser"])
        .spawn()
        .map(drop)
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
    cmd.args(["--renderer", "glow", "--no-chooser"])
        .env(FALLBACK_ENV, "1");
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
fn run(backend: Backend, args: &Args, chooser: Option<ChooserState>) -> eframe::Result<()> {
    let app_options = AppOptions {
        smoke_test: args.smoke_test,
        chooser,
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
