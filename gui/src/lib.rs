#![warn(clippy::all, rust_2018_idioms)]

pub mod launcher;
pub mod renderer;
pub mod settings;
pub mod ui;
pub mod user_config;

pub use ui::PloxApp;

pub const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
