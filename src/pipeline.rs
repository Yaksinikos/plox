//! Front-end agnostic evaluation pipeline: detect game, fetch rules, gather mods, evaluate and sort.

use std::path::PathBuf;

use log::{error, warn};

use crate::{
    ESupportedGame, conflict2, detect_game, download_latest_rules, gather_mods,
    get_default_rules_dir, get_game_version,
    parser::{self, Warning},
    sorter::new_stable_sorter,
};

/// Outcome of the sorting step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ELoadStatus {
    Conflicts,
    Cycle,
    Success,
}

/// Input options for [`run`]
#[derive(Debug, Clone)]
pub struct PipelineOptions {
    /// Game to evaluate, detected from `root` if `None`
    pub game: Option<ESupportedGame>,
    /// Game root folder
    pub root: PathBuf,
    /// Rules folder, the game default if `None`
    pub rules_dir: Option<PathBuf>,
    /// Download the latest rules before parsing
    pub download_rules: bool,
    /// Sort even if conflicts were found
    pub ignore_warnings: bool,
}

/// Result of a pipeline run
#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub game: ESupportedGame,
    /// Game root folder the mods were gathered from
    pub root: PathBuf,
    pub game_version: Option<String>,
    pub old_order: Vec<String>,
    pub new_order: Vec<String>,
    pub warnings: Vec<Warning>,
    /// lowercase-insensitive plugin name to index into `warnings`
    pub plugin_warning_map: Vec<(String, usize)>,
    pub status: ELoadStatus,
}

/// Runs the full evaluation pipeline, reporting progress messages through `progress`.
///
/// # Errors
///
/// Returns an error message if no game is detected, the rules cannot be parsed or sorting fails.
pub fn run(options: &PipelineOptions, progress: impl Fn(&str)) -> Result<PipelineResult, String> {
    let game = if let Some(game) = options.game {
        progress(&format!("Using game: {game:?}"));
        game
    } else if let Some(game) = detect_game() {
        progress(&format!("Detected game: {game:?}"));
        game
    } else {
        return Err("No game detected".to_owned());
    };

    let game_version = get_game_version(game);

    let rules_dir = options
        .rules_dir
        .clone()
        .unwrap_or_else(|| get_default_rules_dir(game));
    if options.download_rules {
        progress("Downloading rules");
        download_latest_rules(game, &rules_dir);
    } else {
        progress("Skipping rules download");
    }

    progress("Gathering mods");
    let mods = gather_mods(&options.root, game, &game_version);

    progress("Initializing parser");
    let mut parser = parser::get_parser(game, game_version.clone());
    parser.parse(&rules_dir).map_err(|e| {
        error!("Parser init failed: {e}");
        format!("Parser init failed: {e}")
    })?;

    progress("Evaluating plugins");
    parser.evaluate_plugins(&mods);
    let warnings = parser.warnings.clone();
    let plugin_warning_map = warnings
        .iter()
        .enumerate()
        .flat_map(|(i, w)| w.get_plugins().into_iter().map(move |p| (p, i)))
        .collect();

    let has_conflicts = !options.ignore_warnings
        && warnings.iter().any(|w| {
            let is_conflict = conflict2(&w.rule).is_some();
            if is_conflict {
                warn!("Conflict detected: {w:?}");
            }
            is_conflict
        });

    let old_order: Vec<String> = mods.iter().map(|m| m.name.clone()).collect();
    let mut new_order = old_order.clone();

    let status = if has_conflicts {
        warn!("Conflicts detected, skipping sort");
        progress("Conflicts detected, skipping sort");
        ELoadStatus::Conflicts
    } else if parser.order_rules.is_empty() {
        ELoadStatus::Success
    } else {
        progress("Sorting mods");
        match new_stable_sorter().topo_sort(game, &mods, &parser.order_rules, &parser.warning_rules)
        {
            Ok(sorted) => {
                new_order = sorted;
                ELoadStatus::Success
            }
            Err(e) if e.contains("Out of iterations") => {
                error!("error sorting: {e}");
                progress("Cycle detected, skipping sort.");
                ELoadStatus::Cycle
            }
            Err(e) => {
                error!("error sorting: {e}");
                return Err(format!("error sorting: {e}"));
            }
        }
    };

    Ok(PipelineResult {
        game,
        root: options.root.clone(),
        game_version,
        old_order,
        new_order,
        warnings,
        plugin_warning_map,
        status,
    })
}
