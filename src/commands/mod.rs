pub mod graph;
pub use graph::*;

pub mod sort;
pub use sort::*;

pub mod verify;
pub use verify::*;

pub mod list;
pub use list::*;

/// Rules dir from the command line, or the default for `game`.
fn rules_dir(game: crate::ESupportedGame, path: Option<&str>) -> std::path::PathBuf {
    path.map_or_else(
        || crate::get_default_rules_dir(game),
        std::path::PathBuf::from,
    )
}

/// Mods from `mod_list` if given, otherwise from the game folder.
/// Returns `None` if no mods were found in the game folder.
#[expect(
    clippy::ref_option,
    reason = "matches the gather_* and read_file_as_list signatures"
)]
fn gather(
    game: crate::ESupportedGame,
    root: &std::path::Path,
    mod_list: Option<&std::path::Path>,
    game_version: &Option<String>,
) -> Option<Vec<crate::PluginData>> {
    if let Some(list) = mod_list {
        return Some(crate::read_file_as_list(list, game_version));
    }
    let mods = match game {
        crate::ESupportedGame::Morrowind => crate::gather_tes3_mods(&root),
        crate::ESupportedGame::Cyberpunk => crate::gather_cp77_mods(&root, game_version),
        crate::ESupportedGame::Openmw => crate::gather_openmw_mods(),
    };
    if mods.is_empty() {
        log::info!("No mods found");
        return None;
    }
    Some(mods)
}

/// Game root from the command line, or the current working directory.
fn game_root(folder: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    folder.map(std::path::Path::to_path_buf).or_else(|| {
        std::env::current_dir()
            .map_err(|e| log::error!("No current working dir: {e}"))
            .ok()
    })
}
