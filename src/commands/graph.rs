use std::path::PathBuf;
use std::process::ExitCode;

use log::error;
use petgraph::dot::{Config, Dot};

use crate::{ESupportedGame, GRAPH_FILE, get_game_version, parser, sorter};

/// Writes the dependency graph of the mods as a dot file
pub fn graph(
    game: ESupportedGame,
    game_folder: &Option<PathBuf>,
    rules_path: &Option<String>,
    mod_list: &Option<PathBuf>,
) -> ExitCode {
    // get game root
    let Some(root) = super::game_root(game_folder.as_deref()) else {
        return ExitCode::FAILURE;
    };

    let rules_dir = super::rules_dir(game, rules_path.as_deref());
    let game_version = get_game_version(game);
    let Some(mods) = super::gather(game, &root, mod_list.as_deref(), &game_version) else {
        return ExitCode::FAILURE;
    };

    let mut parser = parser::get_parser(game, game_version);
    if let Err(e) = parser.parse(rules_dir) {
        error!("Parser init failed: {e}");
        return ExitCode::FAILURE;
    }

    let data = sorter::get_graph_data(&mods, &parser.order_rules, &parser.warning_rules);
    let g = sorter::build_graph(&data);

    let viz = Dot::with_config(&g, &[Config::EdgeNoLabel]);
    if let Err(e) = std::fs::write(GRAPH_FILE, format!("{viz:?}")) {
        error!("Could not write {GRAPH_FILE}: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
