use std::process::ExitCode;

use log::{error, info, warn};

use crate::{ESupportedGame, debug_get_mods_from_order_rules, get_game_version, parser, sorter};

/// Verifies integrity of the specified rules
pub fn verify(game: ESupportedGame, rules_path: &Option<String>) -> ExitCode {
    let rules_dir = super::rules_dir(game, rules_path.as_deref());

    info!("Verifying rules from {} ...", rules_dir.display());

    let game_version = get_game_version(game);
    let mut parser = parser::get_parser(game, game_version);
    if let Err(e) = parser.parse(rules_dir) {
        error!("Parser init failed: {e}");
        return ExitCode::FAILURE;
    }

    if parser.warning_rules.is_empty() {
        warn!("No rules found to evaluate");
        return ExitCode::FAILURE;
    }

    let mods = debug_get_mods_from_order_rules(&parser.order_rules);
    if sorter::new_unstable_sorter()
        .topo_sort(game, &mods, &parser.order_rules, &parser.warning_rules)
        .is_ok()
    {
        info!("Verify SUCCESS");
        ExitCode::SUCCESS
    } else {
        error!("Verify FAILURE");
        ExitCode::FAILURE
    }
}
