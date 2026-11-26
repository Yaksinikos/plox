use std::path::PathBuf;
use std::process::ExitCode;

use log::info;

use crate::{ESupportedGame, gather_mods};

/// Lists the current mod load order
pub fn list_mods(root: &Option<PathBuf>, game: ESupportedGame) -> ExitCode {
    info!("Printing active mods...");

    let Some(root) = super::game_root(root.as_deref()) else {
        return ExitCode::FAILURE;
    };

    for m in gather_mods(&root, game, &None) {
        println!("{}", m.name);
        //info!("{}", m);
    }

    ExitCode::SUCCESS
}
