use std::path::PathBuf;
use std::process::ExitCode;

use log::{debug, error, info, warn};

use crate::{
    ESupportedGame, EWarningRule, PluginData, TWarningRule, download_latest_rules,
    get_game_version, parser, sorter, update_new_load_order,
};

pub struct CliSortOptions {
    pub game: ESupportedGame,
    pub game_folder: Option<PathBuf>,
    pub rules_dir: Option<String>,
    pub mod_list: Option<PathBuf>,
    pub dry_run: bool,
    pub unstable: bool,
    pub no_download: bool,
}

/// Sorts the current mod load order according to specified rules
pub fn sort(options: &CliSortOptions) -> ExitCode {
    let game = options.game;

    let Some(root) = super::game_root(options.game_folder.as_deref()) else {
        return ExitCode::FAILURE;
    };
    let rules_dir = super::rules_dir(game, options.rules_dir.as_deref());

    if options.no_download {
        info!("Skipping downloading latest rules");
    } else {
        download_latest_rules(game, &rules_dir);
    }

    let game_version = get_game_version(game);
    let Some(mods) = super::gather(game, &root, options.mod_list.as_deref(), &game_version) else {
        return ExitCode::FAILURE;
    };

    let mut parser = parser::get_parser(game, game_version);
    if let Err(e) = parser.parse(rules_dir) {
        error!("Parser init failed: {e}");
        return ExitCode::FAILURE;
    }

    if parser.warning_rules.is_empty() {
        warn!("No rules found to evaluate");
    } else {
        info!("Evaluating mod list...\n");
        debug!("{mods:?}");
        parser.evaluate_plugins(&mods);
        for warning in &parser.warnings {
            print_warning(&warning.rule);
        }
    }

    if parser.order_rules.is_empty() {
        warn!("No rules found to sort");
        return ExitCode::SUCCESS;
    }

    info!("Sorting mods...");
    let mut sorter = if options.unstable {
        sorter::new_unstable_sorter()
    } else {
        sorter::new_stable_sorter()
    };

    match sorter.topo_sort(game, &mods, &parser.order_rules, &parser.warning_rules) {
        Ok(result) => apply_result(game, &mods, &result, options.dry_run),
        Err(e) => {
            error!("error sorting: {e:?}");
            ExitCode::FAILURE
        }
    }
}

fn print_warning(rule: &EWarningRule) {
    match rule {
        EWarningRule::Note(n) => {
            info!("[NOTE]\n{}", n.get_comment());
            info!("Reference: [{}]", n.plugins.join(";"));
        }
        EWarningRule::Conflict(c) => {
            warn!("[CONFLICT]\n{}", c.get_comment());
            info!("Reference: [{}]", c.plugins.join(";"));
        }
        EWarningRule::Requires(r) => {
            error!("[REQUIRES]\n{}", r.get_comment());
            info!("Reference: [{}]", r.plugins.join(";"));
        }
        EWarningRule::Patch(p) => {
            warn!("[Patch]\n{}", p.get_comment());
            info!("Reference: [{}]", p.plugins.join(";"));
        }
    }
    println!();
}

/// Reports the sorted order and writes it to disk unless `dry_run` is set.
fn apply_result(
    game: ESupportedGame,
    mods: &[PluginData],
    result: &[String],
    dry_run: bool,
) -> ExitCode {
    let unchanged = mods
        .iter()
        .map(|f| f.name.to_lowercase())
        .eq(result.iter().cloned());

    if dry_run {
        info!("Dry run...");
        debug!("Old:\n{mods:?}");
        debug!("New:\n{result:?}");
    } else {
        info!("Current:\n{mods:?}");
    }

    if unchanged {
        info!("Mods are in correct order, no sorting needed.");
        return ExitCode::SUCCESS;
    }

    if dry_run {
        info!("New order:\n{result:?}");
        return ExitCode::SUCCESS;
    }

    info!("New:\n{result:?}");
    match update_new_load_order(game, result) {
        Ok(()) => {
            info!("Update successful");
            ExitCode::SUCCESS
        }
        Err(e) => {
            error!("Could not update load order: {e}");
            ExitCode::FAILURE
        }
    }
}
