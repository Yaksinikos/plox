//! Renderer-agnostic UI. Nothing in here depends on a specific graphics backend.

mod main_view;
mod renderer_chooser;
mod settings;
mod status_bar;

pub use renderer_chooser::ChooserState;

use std::{
    fs::File,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
};

use log::{LevelFilter, info};
use plox::pipeline::{self, PipelineOptions, PipelineResult};
use simplelog::WriteLogger;

use crate::{
    renderer::Backend,
    settings::{AppSettings, PLOX_CONF_FILE, PLOX_LOG_FILE},
};

#[derive(PartialEq, Eq)]
pub enum EModListView {
    NewOrder,
    LoadOrder,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct PloxApp {
    #[serde(skip)]
    settings: AppSettings,
    #[serde(skip)]
    loading: bool,
    #[serde(skip)]
    app_data: Option<PipelineResult>,

    // filters
    show_notes: bool,
    show_conflicts: bool,
    show_requires: bool,
    show_patches: bool,

    #[serde(skip)]
    settings_open: bool,

    #[serde(skip)]
    mod_list_view: EModListView,
    #[serde(skip)]
    text_filter: String,
    #[serde(skip)]
    plugin_filter: String,
    #[serde(skip)]
    plugin_hover_filter: Vec<String>,

    // ui
    #[serde(skip)]
    async_log: String,
    #[serde(skip)]
    last_message: String,
    #[serde(skip)]
    renderer_info: Option<String>,
    #[serde(skip)]
    smoke_test: bool,
    #[serde(skip)]
    chooser: Option<ChooserState>,
    #[serde(skip)]
    frames: u32,
    // Sender/Receiver for async notifications.
    #[serde(skip)]
    tx: Sender<String>,
    #[serde(skip)]
    rx: Receiver<String>,
    #[serde(skip)]
    tx2: Sender<Option<PipelineResult>>,
    #[serde(skip)]
    rx2: Receiver<Option<PipelineResult>>,
}

impl Default for PloxApp {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();

        Self {
            settings: AppSettings::default(),
            app_data: None,
            loading: false,
            show_notes: true,
            show_conflicts: true,
            show_requires: true,
            show_patches: true,
            settings_open: false,
            mod_list_view: EModListView::NewOrder,
            text_filter: String::new(),
            plugin_filter: String::new(),
            plugin_hover_filter: vec![],
            async_log: String::new(),
            last_message: String::new(),
            renderer_info: None,
            smoke_test: false,
            chooser: None,
            frames: 0,
            rx,
            tx,
            rx2,
            tx2,
        }
    }
}

/// Startup options passed from the launcher
#[derive(Debug, Clone, Default)]
pub struct AppOptions {
    /// Render a few frames without running the pipeline, then close
    pub smoke_test: bool,
    /// Show the renderer dialog before the main view
    pub chooser: Option<ChooserState>,
}

/// Frames rendered before a smoke test closes the window
const SMOKE_TEST_FRAMES: u32 = 3;

impl PloxApp {
    /// Called once before the first frame.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, options: AppOptions) -> Self {
        cc.egui_ctx.set_zoom_factor(1.3);

        // Load previous app state (if any).
        let mut app: Self = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        app.settings = AppSettings::from_file(&PathBuf::from(PLOX_CONF_FILE));
        app.smoke_test = options.smoke_test;
        app.chooser = options.chooser;

        if app.smoke_test {
            app.loading = true;
        } else if app.chooser.is_none() {
            app.start_pipeline(app.settings.ignore_warnings);
        }
        app
    }

    /// Handles the renderer dialog. Returns `true` while it is shown.
    fn show_chooser(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(state) = &mut self.chooser else {
            return false;
        };
        match renderer_chooser::show(state, ui) {
            None => true,
            Some(Backend::Glow) => {
                self.chooser = None;
                self.start_pipeline(self.settings.ignore_warnings);
                true
            }
            Some(Backend::Wgpu) => {
                if let Err(e) = crate::launcher::relaunch(Backend::Wgpu) {
                    log::error!("Could not start the wgpu renderer: {e}");
                    self.chooser = None;
                    self.start_pipeline(self.settings.ignore_warnings);
                } else {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                true
            }
        }
    }

    /// Runs the evaluation pipeline on a worker thread.
    fn start_pipeline(&mut self, ignore_warnings: bool) {
        self.loading = true;
        let tx = self.tx.clone();
        let tx2 = self.tx2.clone();
        let settings = self.settings.clone();

        std::thread::spawn(move || {
            let root = match std::env::current_dir() {
                Ok(root) => root,
                Err(e) => {
                    let _ = tx.send(format!("No current working dir: {e}"));
                    let _ = tx2.send(None);
                    return;
                }
            };
            let options = PipelineOptions {
                game: settings.game,
                root,
                rules_dir: None,
                download_rules: !settings.no_rules_download,
                ignore_warnings,
            };
            let result = match pipeline::run(&options, |msg| {
                let _ = tx.send(msg.to_owned());
            }) {
                Ok(result) => Some(result),
                Err(e) => {
                    let _ = tx.send(e);
                    None
                }
            };
            let _ = tx.send("App initialized".to_owned());
            let _ = tx2.send(result);
        });
    }

    fn poll_worker(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.async_log.push_str(&msg);
            self.async_log.push('\n');
            self.last_message = msg;
        }
        if let Ok(result) = self.rx2.try_recv() {
            self.app_data = result;
            self.loading = false;
        }
    }
}

/// Initializes logging from the `plox.toml` in the current folder.
pub fn init_logger() {
    let settings = AppSettings::from_file(&PathBuf::from(PLOX_CONF_FILE));
    let level = level_from_str(settings.log_level.as_deref().unwrap_or("info"));
    if settings.log_to_file
        && let Ok(file) = File::create(PathBuf::from(PLOX_LOG_FILE))
    {
        let _ = WriteLogger::init(level, simplelog::Config::default(), file);
    } else {
        let _ = simplelog::TermLogger::init(
            level,
            simplelog::Config::default(),
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        );
    }
    info!("PLOX v{}", crate::CARGO_PKG_VERSION);
}

fn level_from_str(log_level: &str) -> LevelFilter {
    match log_level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        "off" => LevelFilter::Off,
        _ => LevelFilter::Info,
    }
}

impl eframe::App for PloxApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.smoke_test {
            return;
        }
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if self.renderer_info.is_none() {
            let info = crate::renderer::active_info(frame);
            info!("Renderer: {info}");
            self.renderer_info = Some(info);
        }
        if self.smoke_test {
            self.frames += 1;
            if self.frames >= SMOKE_TEST_FRAMES {
                info!("Smoke test passed");
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.show_chooser(ui) {
            return;
        }
        self.poll_worker();

        if self.settings_open {
            self.settings_open = settings::show(ui.ctx(), &mut self.settings);
        }

        main_view::menu_bar(self, ui);
        status_bar::show(self, ui);

        if self.loading || self.app_data.is_none() {
            main_view::loading(self, ui);
            // pump ui events while the worker thread runs
            ui.ctx().request_repaint();
            return;
        }

        main_view::show(self, ui);
    }
}
