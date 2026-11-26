pub mod graph;
pub use graph::*;

pub mod sort;
pub use sort::*;

pub mod verify;
pub use verify::*;

pub mod list;
pub use list::*;

/// Game root from the command line, or the current working directory.
fn game_root(folder: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    folder.map(std::path::Path::to_path_buf).or_else(|| {
        std::env::current_dir()
            .map_err(|e| log::error!("No current working dir: {e}"))
            .ok()
    })
}
