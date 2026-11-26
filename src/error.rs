//! Error type of the plox library

use std::path::PathBuf;

/// Errors returned by plox operations
#[derive(Debug, thiserror::Error)]
pub enum PloxError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{action} is not supported for {game}")]
    Unsupported { action: &'static str, game: String },

    #[error("No config file found at {}", .0.display())]
    ConfigNotFound(PathBuf),

    #[error("Rules error: {0}")]
    Rules(String),

    #[error("Sorting error: {0}")]
    Sort(String),
}

pub type Result<T, E = PloxError> = std::result::Result<T, E>;
