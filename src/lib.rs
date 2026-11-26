use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{env, vec};

use clap::ValueEnum;

pub mod commands;
pub use commands::*;

pub mod error;
pub use error::PloxError;

pub mod expressions;
pub mod parser;
pub mod pipeline;
pub mod rules;
pub mod sorter;

use byteorder::{LittleEndian, ReadBytesExt};
use ini::Ini;
use log::{error, info, warn};
use regex::Regex;
use rules::{
    Conflict, EOrderRule, ERule, EWarningRule, NearEnd, NearStart, Note, Order, Patch, Requires,
    TParser, TWarningRule,
};
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ELogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

#[must_use]
pub fn log_level_to_str(level: ELogLevel) -> String {
    match level {
        ELogLevel::Trace => "trace".into(),
        ELogLevel::Debug => "debug".into(),
        ELogLevel::Info => "info".into(),
        ELogLevel::Warn => "warn".into(),
        ELogLevel::Error => "error".into(),
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, Serialize, Deserialize)]
pub enum ESupportedGame {
    Morrowind,
    Openmw,
    Cyberpunk,
}
pub const PLOX_RULES_BASE: &str = "plox_base.txt";
pub const GRAPH_FILE: &str = "graphviz.dot";

////////////////////////////////////////////////////////////////////////
// GAMES
////////////////////////////////////////////////////////////////////////

/// Returns the entry `name` in `dir`, matching the name case-insensitively.
///
/// Prefers an exact match. Falls back to `dir/name` if no entry matches, so the result can
/// still be used for error messages or for creating the file.
#[must_use]
pub fn find_case_insensitive(dir: &Path, name: &str) -> PathBuf {
    let exact = dir.join(name);
    if exact.exists() {
        return exact;
    }
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    fs::read_dir(dir)
        .ok()
        .and_then(|entries| {
            entries
                .map_while(Result::ok)
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        })
        .map_or(exact, |e| e.path())
}

/// Detect game from current working directory
#[must_use]
pub fn detect_game() -> Option<ESupportedGame> {
    let here = Path::new(".");
    if find_case_insensitive(here, "Morrowind.exe").exists()
        || find_case_insensitive(here, "Morrowind.ini").exists()
    {
        Some(ESupportedGame::Morrowind)
    } else if PathBuf::from("openmw.cfg").exists()
        || openmw_config::try_default_user_config_file().is_ok_and(|p| p.is_file())
        || openmw_config::try_default_global_config_path()
            .is_ok_and(|p| p.join("openmw.cfg").is_file())
    {
        Some(ESupportedGame::Openmw)
    } else if PathBuf::from("bin")
        .join("x64")
        .join("Cyberpunk2077.exe")
        .exists()
    {
        Some(ESupportedGame::Cyberpunk)
    } else {
        None
    }
}

/// Detect game from current working directory and read version from exe
/// Does not log errors
fn read_pe_version(path: &PathBuf) -> Option<String> {
    use pelite::pe64::{Pe, PeFile};

    if let Ok(map) = pelite::FileMap::open(path)
        && let Ok(file) = PeFile::from_bytes(&map)
        && let Ok(resources) = file.resources()
        && let Ok(version_info) = resources.version_info()
        && let Some(language) = version_info.translation().first()
        && let Some(version) = version_info.value(*language, "ProductVersion")
    {
        return Some(version);
    }

    None
}

/// Detect game from current working directory and read version from exe
#[must_use]
pub fn get_game_version(game: ESupportedGame) -> Option<String> {
    match game {
        ESupportedGame::Morrowind | ESupportedGame::Openmw => None,
        ESupportedGame::Cyberpunk => {
            let exe_path = PathBuf::from("bin").join("x64").join("Cyberpunk2077.exe");
            if exe_path.exists() {
                return read_pe_version(&exe_path);
            }
            None
        }
    }
}

/// flattens a list of ordered mod pairs into a list of mod names
#[must_use]
pub fn debug_get_mods_from_order_rules(order_rules: &[EOrderRule]) -> Vec<PluginData> {
    debug_get_mods_from_ordering(&get_ordering_from_order_rules(order_rules))
}

/// flattens a list of ordered mod pairs into a list of mod names
#[must_use]
pub fn debug_get_mods_from_ordering(order: &[(String, String)]) -> Vec<PluginData> {
    let mut result: Vec<PluginData> = vec![];
    for (a, b) in order {
        for a in [a, b] {
            let name = if a.contains('*') || a.contains('?') || a.contains("<ver>") {
                // Wildcards
                a.replace('?', "x")
                    .replace(['*'], "")
                    .replace("<ver>", "1.0")
            } else {
                a.to_owned()
            };

            let data = PluginData::new(name, 0);
            if !result.contains(&data) {
                result.push(data);
            }
        }
    }

    result.dedup();

    result
}

/// Name of the plox folder inside the platform directories
pub const PLOX_APP_DIR: &str = "plox";

/// The plox folder in the platform config dir
/// (`~/.config/plox`, `%APPDATA%\plox`, `~/Library/Application Support/plox`)
#[must_use]
pub fn user_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(PLOX_APP_DIR))
}

/// The plox folder in the platform data dir
/// (`~/.local/share/plox`, `%APPDATA%\plox`, `~/Library/Application Support/plox`)
#[must_use]
pub fn user_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(PLOX_APP_DIR))
}

/// Returns `name` in the current folder if it exists there, otherwise in `platform_dir`.
/// Falls back to the current folder if there is no platform dir.
#[must_use]
pub fn local_or_platform(name: &str, platform_dir: Option<PathBuf>) -> PathBuf {
    let local = PathBuf::from(name);
    if local.exists() {
        return local;
    }
    platform_dir.map_or(local, |d| d.join(name))
}

/// Gets the default rules dir for a game.
///
/// A rules folder (`mlox` or `plox`) in the current folder is used first,
/// otherwise the one in the platform data dir.
#[must_use]
pub fn get_default_rules_dir(game: ESupportedGame) -> PathBuf {
    let name = match game {
        ESupportedGame::Morrowind | ESupportedGame::Openmw => "mlox",
        ESupportedGame::Cyberpunk => "plox",
    };
    local_or_platform(name, user_data_dir())
}

/// Download latest rules from the internet
pub fn download_latest_rules(game: ESupportedGame, rules_dir: &PathBuf) {
    match game {
        ESupportedGame::Morrowind | ESupportedGame::Openmw => download_mlox_rules(rules_dir),
        ESupportedGame::Cyberpunk => download_plox_rules(rules_dir),
    }
}

fn download_file<P>(url: &str, output_path: &P) -> Result<(), Box<dyn Error>>
where
    P: AsRef<Path>,
{
    // Send an HTTP GET request to the URL
    let response = reqwest::blocking::get(url)?;

    // get response body
    let body = &mut response.bytes()?;

    // hash check
    let hash_path = output_path.as_ref().with_extension("hash");
    if hash_path.exists() {
        // check against remote hash
        let local_hash_bytes = fs::read(&hash_path)?;
        let local_hash_str = String::from_utf8_lossy(&local_hash_bytes).to_string();
        if let Ok(local_hash) = local_hash_str.parse::<u64>() {
            let remote_hash = seahash::hash(body);
            if local_hash == remote_hash {
                // return
                info!(
                    "File already is latest version: {}",
                    output_path.as_ref().display()
                );
                return Ok(());
            }
        }
    }

    // Create a file and write

    let mut file = File::create(output_path)?;
    io::copy(&mut body.as_ref(), &mut file)?;
    info!(
        "File downloaded successfully: {}",
        output_path.as_ref().display()
    );

    // create hash
    let remote_hash = seahash::hash(body);
    fs::write(hash_path, remote_hash.to_string())?;

    Ok(())
}

fn download_mlox_rules(rules_dir: &PathBuf) {
    match fs::create_dir_all(rules_dir) {
        Ok(()) => {
            // download
            let repo = "https://github.com/DanaePlays/mlox-rules/raw/main/";
            let files = ["mlox_base.txt", "mlox_user.txt"];
            for file in files {
                let output_path = rules_dir.join(file); // Specify the output path here
                let url = repo.to_owned() + file;
                match download_file(&url, &output_path) {
                    Ok(()) => {}
                    Err(err) => error!("Error downloading file: {err}"),
                }
            }
        }
        Err(e) => {
            error!(
                "Could not create rules directory at {}: {}",
                rules_dir.display(),
                e
            );
        }
    }
}

fn download_plox_rules(rules_dir: &PathBuf) {
    match fs::create_dir_all(rules_dir) {
        Ok(()) => {
            // download
            let repo = "https://github.com/rfuzzo/cmop-rules/raw/main/";
            let files = ["plox_base.txt"];
            for file in files {
                let output_path = rules_dir.join(file); // Specify the output path here
                let url = repo.to_owned() + file;
                match download_file(&url, &output_path) {
                    Ok(()) => {}
                    Err(err) => error!("Error downloading file: {err}"),
                }
            }
        }
        Err(e) => {
            error!(
                "Could not create rules directory at {}: {}",
                rules_dir.display(),
                e
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct PluginData {
    pub name: String,
    pub size: u64,

    pub description: Option<String>,
    pub version: Option<semver::Version>,
    pub game_version: Option<semver::Version>,
    pub masters: Option<Vec<(String, u64)>>,
}

impl PluginData {
    #[must_use]
    pub const fn new(name: String, size: u64) -> Self {
        Self {
            name,
            size,
            description: None,
            game_version: None,
            version: None,
            masters: None,
        }
    }
}

/// Gets a list of mod names from the game root folder
///
/// # Errors
///
/// This function will return an error if IO operations fail
pub fn gather_mods<P>(
    root: &P,
    game: ESupportedGame,
    game_version: &Option<String>,
) -> Vec<PluginData>
where
    P: AsRef<Path>,
{
    match game {
        ESupportedGame::Morrowind => gather_tes3_mods(root),
        ESupportedGame::Cyberpunk => gather_cp77_mods(root, game_version),
        ESupportedGame::Openmw => gather_openmw_mods(),
    }
}

/// Get all plugins (esp, omwaddon, omwscripts) in a folder
fn get_plugins_in_folder<P>(path: &P, use_omw_plugins: bool) -> Vec<PathBuf>
where
    P: AsRef<Path>,
{
    // get all plugins
    let mut results: Vec<PathBuf> = vec![];
    if let Ok(plugins) = fs::read_dir(path) {
        plugins.for_each(|p| {
            if let Ok(file) = p {
                let file_path = file.path();
                if !file_path.is_dir()
                    && let Some(ext_os) = file_path.extension()
                {
                    let ext = ext_os.to_ascii_lowercase();
                    if ext == "esm"
                        || ext == "esp"
                        || (use_omw_plugins && ext == "omwaddon")
                        || (use_omw_plugins && ext == "omwscripts")
                        || (use_omw_plugins && ext == "omwgame")
                    {
                        results.push(file_path);
                    }
                }
            }
        });
    }
    results
}

fn get_plugins_sorted<P>(path: &P, use_omw_plugins: bool) -> Vec<PathBuf>
where
    P: AsRef<Path>,
{
    // get plugins
    let mut plugins = get_plugins_in_folder(path, use_omw_plugins);

    // sort
    // unreadable timestamps sort first
    plugins.sort_by_cached_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    plugins
}

pub fn gather_tes3_mods<P>(path: &P) -> Vec<PluginData>
where
    P: AsRef<Path>,
{
    let root = path.as_ref();
    let files = get_plugins_sorted(&find_case_insensitive(root, "Data Files"), false);
    let names = files.iter().filter_map(|f| map_data(f)).collect::<Vec<_>>();

    // check against mw ini
    let morrowind_ini_path = find_case_insensitive(root, "Morrowind.ini");
    if morrowind_ini_path.exists() {
        // parse ini
        if let Ok(ini) = Ini::load_from_file(morrowind_ini_path) {
            let mut final_files: Vec<PluginData> = vec![];
            if let Some(section) = ini.section(Some("Game Files")) {
                let mods_in_ini: Vec<String> = section.iter().map(|f| f.1.to_lowercase()).collect();
                for data in names {
                    if mods_in_ini.contains(&data.name.to_lowercase()) {
                        final_files.push(data);
                    }
                }

                return final_files;
            }
            warn!(
                "Morrowind.ini found but no [Game Files] section, using all plugins in Data Files"
            );
        } else {
            error!("Morrowind.ini could not be read");
        }
    } else {
        warn!("No Morrowind.ini found, using all plugins in Data Files");
    }

    info!("Found {} active plugins", names.len());
    names
}

/// Loads the `OpenMW` configuration chain.
///
/// An `openmw.cfg` in the current directory is used first, then the `OpenMW` discovery
/// (`OPENMW_CONFIG`, `OPENMW_CONFIG_DIR`, root and global config, then the user config).
///
/// # Errors
///
/// Returns an error if no `openmw.cfg` is found or the chain cannot be parsed.
pub fn load_openmw_config() -> io::Result<openmw_config::OpenMWConfiguration> {
    let local = PathBuf::from("openmw.cfg");
    let config = if local.is_file() {
        openmw_config::OpenMWConfiguration::new(Some(env::current_dir()?))
    } else {
        openmw_config::OpenMWConfiguration::from_env_or_user_config()
    };
    config.map_err(io::Error::other)
}

/// Finds `name` in the `data=` directories. Later directories override earlier ones.
fn find_in_data_dirs(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    let lower = name.to_lowercase();
    dirs.iter().rev().find_map(|dir| {
        let exact = dir.join(name);
        if exact.is_file() {
            return Some(exact);
        }
        fs::read_dir(dir).ok()?.map_while(Result::ok).find_map(|e| {
            let path = e.path();
            (path.is_file() && e.file_name().to_string_lossy().to_lowercase() == lower)
                .then_some(path)
        })
    })
}

#[must_use]
pub fn gather_openmw_mods() -> Vec<PluginData> {
    match load_openmw_config() {
        Ok(cfg) => openmw_mods(&cfg),
        Err(e) => {
            error!("No usable openmw.cfg found: {e}");
            vec![]
        }
    }
}

/// The `content=` files of `cfg`, looked up in its `data=` directories
#[must_use]
pub fn openmw_mods(cfg: &openmw_config::OpenMWConfiguration) -> Vec<PluginData> {
    let dirs = cfg
        .data_directories_iter()
        .map(|d| d.parsed().to_path_buf())
        .collect::<Vec<_>>();

    let mut names = vec![];
    for file in cfg.content_files_iter() {
        let name = file.value_str();
        if let Some(path) = find_in_data_dirs(&dirs, name) {
            if let Some(data) = map_data(&path) {
                names.push(data);
            }
        } else {
            warn!("Content file {name} not found in any data directory");
        }
    }

    info!("Found {} active plugins", names.len());
    names
}

fn map_data(f: &Path) -> Option<PluginData> {
    if let Some(file_name) = f.file_name().and_then(|n| n.to_str()) {
        let mut data = PluginData {
            name: file_name.to_owned(),
            size: f.metadata().map_or(0, |m| m.len()),
            description: None,
            version: None,
            masters: None,
            game_version: None, // TODO add game version
        };

        match parse_header(f) {
            Ok(header) => {
                data.description = Some(header.description);
                data.masters = header.masters;
            }
            Err(e) => {
                log::debug!("Error parsing header: {}, {}", e, f.display());
            }
        }

        // parse semver
        if let Some(version) = get_version(file_name, &data.description) {
            data.version = Some(version);
        }

        return Some(data);
    }
    None
}

const VERSION_REGEX: &str = r"(\d+(?:[_.-]?\d+)*[a-zA-Z]?)";

/// Get version from filename or description
#[must_use]
pub fn get_version(file_name: &str, description: &Option<String>) -> Option<Version> {
    // description first, then the filename
    let final_version_str = description
        .as_deref()
        .and_then(match_desc_version)
        .or_else(|| match_filename_version(file_name));

    if let Some(version) = final_version_str
        && let Some(value) = get_semver(version.as_str())
    {
        return Some(value);
    }

    None
}

/// Get semver from string
fn get_semver(version: &str) -> Option<Version> {
    // replace _ and - with .
    let formatted_version = version.replace(['_', '-'], ".");
    // TODO limit to major minor and patch version

    match lenient_semver::parse(&formatted_version) {
        Ok(v) => return Some(v),
        Err(e) => {
            log::debug!("Error parsing version: {e}");
        }
    }
    None
}

/// Get version from filename
///
/// # Panics
///
/// Panics if the regex pattern is invalid
fn match_filename_version(file_name: &str) -> Option<String> {
    static PATTERN: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(&format!(r"\D{VERSION_REGEX}\D*")).ok());
    if let Some(captures) = PATTERN.as_ref()?.captures(file_name)
        && let Some(version) = captures.get(1)
    {
        return Some(version.as_str().to_string());
    }
    None
}

/// Get version from description
///
/// # Panics
///
/// Panics if the regex pattern is invalid
fn match_desc_version(desc: &str) -> Option<String> {
    static PATTERN: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::new(&format!(
            r"\b(?:version\b\D+|v(?:er)?\.?\s*){VERSION_REGEX}"
        ))
        .ok()
    });
    if let Some(captures) = PATTERN.as_ref()?.captures(desc)
        && let Some(version) = captures.get(1)
    {
        return Some(version.as_str().to_string());
    }
    None
}

fn read_file_to_vec(file_path: &PathBuf) -> io::Result<Vec<String>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

    Ok(lines)
}

pub fn gather_cp77_mods<P>(root: &P, game_version: &Option<String>) -> Vec<PluginData>
where
    P: AsRef<Path>,
{
    // gather mods from archive/pc/mod
    let archive_path = root.as_ref().join("archive").join("pc").join("mod");

    // parse version as semver
    let mut game_version_semver = None;
    if let Some(version) = game_version {
        if let Ok(v) = lenient_semver::parse(version) {
            game_version_semver = Some(v);
        } else {
            error!("Invalid game version: {version}");
        }
    }

    if let Ok(plugins) = fs::read_dir(&archive_path) {
        let mut mods: Vec<PathBuf> = plugins
            .map(|res| res.map(|e| e.path()))
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        // load order
        mods.sort_by(|a, b| {
            a.to_string_lossy()
                .as_bytes()
                .cmp(b.to_string_lossy().as_bytes())
        });

        // load according to modlist.txt
        let mut final_order: Vec<PathBuf> = vec![];
        let modlist_name = "modlist.txt";
        if let Ok(lines) = read_file_to_vec(&archive_path.join(modlist_name)) {
            for name in lines {
                let file_name = archive_path.join(name);
                if mods.contains(&file_name) {
                    final_order.push(file_name);
                }
            }
            // add remaining mods last
            for m in &mods {
                if !final_order.contains(m) {
                    final_order.push(m.clone());
                }
            }
        } else {
            final_order = mods;
        }

        // TODO Redmods

        let vms = final_order
            .iter()
            .filter_map(|e| {
                if !e.is_dir()
                    && let Some(os_ext) = e.extension()
                    && let Some(ext) = os_ext.to_ascii_lowercase().to_str()
                    && ext.contains("archive")
                    && let Some(file_name) = e.file_name().and_then(|n| n.to_str())
                {
                    let data = PluginData {
                        name: file_name.to_owned(),
                        size: e.metadata().map_or(0, |m| m.len()),
                        description: None,
                        version: None,
                        masters: None,
                        game_version: game_version_semver.clone(),
                    };
                    return Some(data);
                }
                None
            })
            .collect::<Vec<_>>();

        return vms;
    }

    vec![]
}

/// Update on disk
///
/// # Errors
///
/// Returns an error if the load order cannot be written.
pub fn update_new_load_order(
    game: ESupportedGame,
    root: &Path,
    result: &[String],
) -> std::io::Result<()> {
    match game {
        ESupportedGame::Morrowind => update_tes3(root, result, false),
        ESupportedGame::Openmw => update_openmw(result),
        ESupportedGame::Cyberpunk => update_cp77(result),
    }
}

fn update_cp77(_result: &[String]) -> std::io::Result<()> {
    // TODO CP77 update mods
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Updating the Cyberpunk 2077 load order is not supported yet",
    ))
}

fn update_openmw(result: &[String]) -> std::io::Result<()> {
    // only the user openmw.cfg is written; parent configs get a `replace=content` there if needed
    let mut cfg = load_openmw_config()?;
    cfg.set_content_files(Some(result.to_vec()));
    cfg.save_user().map_err(io::Error::other)?;
    info!(
        "Saved load order to {}",
        cfg.user_config_path().join("openmw.cfg").display()
    );
    Ok(())
}

/// Writes the load order to `Morrowind.ini` in `root` and redates the plugins in `Data Files`.
///
/// Plugin names are matched case-insensitively and written with their case on disk.
///
/// # Errors
///
/// Returns an error if the ini cannot be written or plugins cannot be redated.
pub fn update_tes3(root: &Path, result: &[String], no_redate: bool) -> std::io::Result<()> {
    let data_files = find_case_insensitive(root, "Data Files");
    let on_disk: HashMap<String, String> = fs::read_dir(&data_files)
        .map(|entries| {
            entries
                .map_while(Result::ok)
                .map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    (name.to_lowercase(), name)
                })
                .collect()
        })
        .unwrap_or_default();
    let names = result
        .iter()
        .map(|f| on_disk.get(&f.to_lowercase()).unwrap_or(f).clone())
        .collect::<Vec<_>>();
    let files = names.iter().map(|f| data_files.join(f)).collect::<Vec<_>>();

    // in tes3 we first update the ini with the new order (this is technically not important but we might as well)
    let morrowind_ini_path = find_case_insensitive(root, "Morrowind.ini");
    if morrowind_ini_path.exists() {
        // parse ini
        let mut buf = Vec::new();
        for line in read_lines(&morrowind_ini_path)?.map_while(Result::ok) {
            // skip plugin lines
            if line.starts_with("[Game Files]") {
                continue;
            }
            if line.starts_with("GameFile") {
                continue;
            }
            writeln!(buf, "{line}")?;
        }

        // add filenames
        writeln!(buf, "[Game Files]")?;
        for (i, r) in names.iter().enumerate() {
            writeln!(buf, "GameFile{i}={r}")?;
        }

        // save
        let mut file = File::create(morrowind_ini_path)?;
        file.write_all(&buf)?;
    } else {
        warn!("No Morrowind.ini found, using all plugins in Data Files");
    }

    if !no_redate {
        redate_mods(&files)?;
    }

    Ok(())
}

/// Checks if the list of mods is in the correct order
#[must_use]
pub fn check_order(result: &[String], order_rules: &[EOrderRule]) -> bool {
    let order = get_ordering_from_order_rules(order_rules);
    let pairs = order;
    for (a, b) in pairs {
        if let Some(results_for_a) = wild_contains(result, &a)
            && let Some(results_for_b) = wild_contains(result, &b)
        {
            for i in &results_for_a {
                for j in &results_for_b {
                    let pos_a = result.iter().position(|x| x == i);
                    let pos_b = result.iter().position(|x| x == j);
                    if let (Some(pos_a), Some(pos_b)) = (pos_a, pos_b)
                        && pos_a > pos_b
                    {
                        return false;
                    }
                }
            }
        }
    }

    true
}

////////////////////////////////////////////////////////////////////////
/// TES3
////////////////////////////////////////////////////////////////////////
#[derive(Debug, Clone, Default)]
pub struct Tes3Header {
    pub description: String,
    pub masters: Option<Vec<(String, u64)>>,
}

/// Reads the TES3 header of a plugin.
///
/// # Errors
///
/// Returns an error if the file cannot be read or is not a TES3 plugin.
pub fn parse_header(f: &Path) -> std::io::Result<Tes3Header> {
    let magic = u32::from_le_bytes(*b"TES3");
    // read file to binary reader
    let mut reader = std::io::BufReader::new(std::fs::File::open(f)?);
    // read first 4 bytes and check magic
    let file_magic = reader.read_u32::<LittleEndian>()?;
    if file_magic != magic {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a valid TES3 plugin",
        ));
    }

    // next 4 bytes is the size of the header
    let header_size = reader.read_u32::<LittleEndian>()?;
    // skip 8 bytes
    reader.seek(std::io::SeekFrom::Current(8))?;
    // read the header
    let mut header_buffer = vec![0; header_size as usize];
    reader.read_exact(&mut header_buffer)?;

    let mut reader = std::io::Cursor::new(header_buffer);
    let header = parse_hedr(&mut reader, u64::from(header_size))?;
    Ok(header)
}

fn parse_hedr<R: Read + Seek>(reader: &mut R, stream_size: u64) -> std::io::Result<Tes3Header> {
    let magic = u32::from_le_bytes(*b"HEDR");
    // check magic
    let file_magic = reader.read_u32::<LittleEndian>()?;

    if file_magic != magic {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a valid TES3 plugin",
        ));
    }

    let mut header = Tes3Header::default();

    // next 4 bytes is the size of the header
    let _header_size = reader.read_u32::<LittleEndian>()?;

    // next 4 bytes is the version
    let _ = reader.read_f32::<LittleEndian>()?;

    // next 4 bytes is unused
    let _ = reader.read_u32::<LittleEndian>()?;

    // read 32 bytes as string
    let mut string_buffer = [0; 32];
    reader.read_exact(&mut string_buffer)?;
    let _author = String::from_utf8_lossy(&string_buffer).to_string();

    // read 256 bytes as string
    let mut string_buffer = [0; 256];
    reader.read_exact(&mut string_buffer)?;
    header.description = String::from_utf8_lossy(&string_buffer)
        .trim_end_matches('\0')
        .to_string();

    // read 4 bytes as u32
    let _num_records = reader.read_u32::<LittleEndian>()?;

    let master_magic = u32::from_le_bytes(*b"MAST");
    let data_magic = u32::from_le_bytes(*b"DATA");

    // read masters
    if reader.stream_position()? >= stream_size {
        return Ok(header);
    }

    let mut masters = vec![];
    loop {
        let magic = reader.read_u32::<LittleEndian>()?;
        if magic == master_magic {
            // next 4 bytes is the size of the master string name
            let master_size = reader.read_u32::<LittleEndian>()?;
            // read master name
            let mut master_buffer = vec![0; master_size as usize];
            reader.read_exact(&mut master_buffer)?;
            let master_name = String::from_utf8_lossy(&master_buffer).to_string();

            // read data magic
            let magic_data = reader.read_u32::<LittleEndian>()?;
            if magic_data != data_magic {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Not a valid TES3 plugin",
                ));
            }
            // next 4 bytes is the size of the master data
            let master_data_size = reader.read_u32::<LittleEndian>()?;
            // verify master data size is 8
            if master_data_size != 8 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Not a valid TES3 plugin",
                ));
            }

            // next 8 bytes is size
            let size = reader.read_u64::<LittleEndian>()?;

            masters.push((master_name.trim_end_matches('\0').to_string(), size));

            // break out if end of stream
            if reader.stream_position()? >= stream_size {
                break;
            }
        } else {
            break;
        }
    }
    header.masters = Some(masters);

    Ok(header)
}

fn set_file_mtime(path: &Path, unix_secs: u64) -> io::Result<()> {
    let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(unix_secs);
    File::options()
        .write(true)
        .open(path)?
        .set_times(fs::FileTimes::new().set_modified(modified))
}

fn redate_mods(files: &[PathBuf]) -> Result<(), io::Error> {
    let fixed_file_times: HashMap<String, usize> = HashMap::from([
        ("morrowind.esm".into(), 1_024_695_106),
        ("tribunal.esm".into(), 1_035_940_926),
        ("bloodmoon.esm".into(), 1_051_807_050),
    ]);
    let start_time = 1_024_695_106;
    let mut current_time = start_time;
    for mod_path in files {
        // Change the modification times of plugin files to be in order of file list, oldest to newest
        // check if is a fixed file time file
        let filename = mod_path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if let Some(time) = fixed_file_times.get(&filename) {
            current_time = *time as u64;
        } else {
            // set the time to start time + 60
            current_time += 60;
        }
        set_file_mtime(mod_path, current_time)?;
    }

    Ok(())
}

////////////////////////////////////////////////////////////////////////
// HELPERS
////////////////////////////////////////////////////////////////////////

fn generate_pair_permutations(input: &[String]) -> Vec<(String, String)> {
    let mut permutations = Vec::new();
    for i in 0..input.len() - 1 {
        for j in i + 1..input.len() {
            permutations.push((input[i].clone(), input[j].clone()));
        }
    }
    permutations
}

/// Extracts a list of ordering-pairs from the order rules
#[must_use]
pub fn get_ordering(rules: &Vec<ERule>) -> Vec<(String, String)> {
    let mut orders: Vec<(String, String)> = vec![];

    for r in rules {
        if let ERule::EOrderRule(EOrderRule::Order(o)) = r {
            // Rule with only one element is an error
            if o.names.len() < 2 {
                continue;
            }
            orders.extend(generate_pair_permutations(&o.names));
        }
    }

    orders
}

/// Extracts a list of ordering-pairs from the order rules
#[must_use]
pub fn get_ordering_from_order_rules(rules: &[EOrderRule]) -> Vec<(String, String)> {
    let mut orders: Vec<(String, String)> = vec![];

    for r in rules {
        if let EOrderRule::Order(o) = r {
            // Rule with only one element is an error
            if o.names.len() < 2 {
                continue;
            }
            orders.extend(generate_pair_permutations(&o.names));
        }
    }

    orders
}

/// Extracts a list of ordering-pairs from the order rules
#[must_use]
pub fn get_ordering_from_orders(rules: &Vec<Order>) -> Vec<(String, String)> {
    let mut orders: Vec<(String, String)> = vec![];

    for o in rules {
        // Rule with only one element is an error
        if o.names.len() < 2 {
            continue;
        }
        orders.extend(generate_pair_permutations(&o.names));
    }

    orders
}

#[must_use]
pub fn is_current_directory_name(name_to_check: &str) -> bool {
    // Get the current directory
    if let Ok(current_dir) = env::current_dir() {
        // Extract the directory name
        if let Some(dir_name) = current_dir.file_name() {
            // Convert the directory name to a string
            if let Some(name) = dir_name.to_str() {
                return name == name_to_check;
            }
        }
    }

    false
}

/// Returns an Iterator to the Reader of the lines of the file.
///
/// # Errors
///
/// This function will return an error if file io fails
pub fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

/// read file line by line into vector
pub fn read_file_as_list<P>(modlist_path: P, game_version: &Option<String>) -> Vec<PluginData>
where
    P: AsRef<Path>,
{
    // parse version as semver
    let mut game_version_semver = None;
    if let Some(version) = game_version {
        if let Ok(v) = lenient_semver::parse(version) {
            game_version_semver = Some(v);
        } else {
            error!("Invalid game version: {version}");
        }
    }

    let mut result: Vec<PluginData> = vec![];
    if let Ok(lines) = read_lines(modlist_path) {
        for line in lines.map_while(Result::ok) {
            let data = PluginData {
                name: line,
                size: 0, // TODO fix dummy size
                description: None,
                version: None,
                masters: None,
                game_version: game_version_semver.clone(),
            };
            result.push(data);
        }
    }
    result
}

/// Whether `pattern` contains wildcards (`*`, `?` or `<ver>`)
fn is_wildcard(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains("<ver>")
}

/// Builds an anchored regex from a wildcard pattern.
///
/// Every regex metacharacter in `pattern` matches literally, except `*` (any sequence),
/// `?` (any single character) and `<ver>` (a version number such as `1.2.3a`, `1.0`, `1`, `1a`, `1_3a` or `77g`).
#[must_use]
pub fn wildcard_regex(pattern: &str) -> Option<Regex> {
    let escaped = regex::escape(pattern)
        .replace(r"\*", ".*")
        .replace(r"\?", ".")
        .replace("<ver>", r"(\d+(?:[_.-]?\d+)*[a-z]?)");
    Regex::new(&format!("^{escaped}$"))
        .map_err(|e| log::error!("Could not construct wildcard pattern for {pattern}: {e}"))
        .ok()
}

/// Checks if the list contains the str
#[must_use]
pub fn wild_contains(list: &[String], str: &String) -> Option<Vec<String>> {
    if is_wildcard(str) {
        let regex = wildcard_regex(str)?;
        let results: Vec<String> = list
            .iter()
            .filter(|item| regex.is_match(item))
            .cloned()
            .collect();
        return (!results.is_empty()).then_some(results);
    }

    list.contains(str).then(|| vec![str.to_owned()])
}

/// Checks if the list contains the str
#[must_use]
pub fn wild_contains_data(list: &[PluginData], str: &str) -> Option<Vec<PluginData>> {
    let str = str.to_lowercase();
    if is_wildcard(&str) {
        let regex = wildcard_regex(&str)?;
        let results: Vec<PluginData> = list
            .iter()
            .filter(|item| regex.is_match(&item.name))
            .cloned()
            .collect();
        return (!results.is_empty()).then_some(results);
    }

    list.iter()
        .find(|f| f.name.to_lowercase() == str)
        .map(|r| vec![r.to_owned()])
}

#[must_use]
pub fn note(f: ERule) -> Option<Note> {
    match f {
        ERule::EWarningRule(EWarningRule::Note(n)) => Some(n),
        _ => None,
    }
}

#[must_use]
pub fn conflict(f: ERule) -> Option<Conflict> {
    match f {
        ERule::EWarningRule(EWarningRule::Conflict(n)) => Some(n),
        _ => None,
    }
}
#[must_use]
pub fn conflict2(f: &EWarningRule) -> Option<Conflict> {
    match f {
        EWarningRule::Conflict(n) => Some(n.clone()),
        _ => None,
    }
}
#[must_use]
pub fn requires(f: ERule) -> Option<Requires> {
    match f {
        ERule::EWarningRule(EWarningRule::Requires(n)) => Some(n),
        _ => None,
    }
}
#[must_use]
pub fn patch(f: ERule) -> Option<Patch> {
    match f {
        ERule::EWarningRule(EWarningRule::Patch(n)) => Some(n),
        _ => None,
    }
}

// order
#[must_use]
pub fn order(f: ERule) -> Option<Order> {
    match f {
        ERule::EOrderRule(EOrderRule::Order(o)) => Some(o),
        _ => None,
    }
}
#[must_use]
pub fn order2(f: EOrderRule) -> Option<Order> {
    match f {
        EOrderRule::Order(o) => Some(o),
        _ => None,
    }
}
#[must_use]
pub fn nearstart(f: ERule) -> Option<NearStart> {
    match f {
        ERule::EOrderRule(EOrderRule::NearStart(o)) => Some(o),
        _ => None,
    }
}
#[must_use]
pub fn nearstart2(f: &EOrderRule) -> Option<NearStart> {
    match f {
        EOrderRule::NearStart(o) => Some(o.clone()),
        _ => None,
    }
}
#[must_use]
pub fn nearend(f: ERule) -> Option<NearEnd> {
    match f {
        ERule::EOrderRule(EOrderRule::NearEnd(o)) => Some(o),
        _ => None,
    }
}
#[must_use]
pub fn nearend2(f: &EOrderRule) -> Option<NearEnd> {
    match f {
        EOrderRule::NearEnd(o) => Some(o.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //use std::fs::create_dir_all;

    use semver::{BuildMetadata, Prerelease};

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_generate_pair_permutations() {
        {
            let input = ["a".to_owned(), "b".to_owned(), "c".to_owned()];
            let got = generate_pair_permutations(&input);
            let expected = [
                ("a".to_owned(), "b".to_owned()),
                ("a".to_owned(), "c".to_owned()),
                ("b".to_owned(), "c".to_owned()),
            ];
            assert_eq!(got, expected);
        }

        {
            let input = ["a".to_owned(), "b".to_owned()];
            let got = generate_pair_permutations(&input);
            let expected = [("a".to_owned(), "b".to_owned())];
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn test_wildcard_matches_star() {
        let pattern = "Hold it - replacer*.esp".to_lowercase();

        {
            let input = "Hold it - replacer.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        {
            let input = "Hold it - replacerA.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        {
            let input = "Hold it - replacerAA.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        // Fails
        {
            let input = "Hold it - replace.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_none());
        }
    }

    #[test]
    fn test_wildcard_matches_questionmark() {
        let pattern = "Rem_LoC?.esp".to_lowercase();

        {
            let input = "Rem_LoCA.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        {
            let input = "Rem_LoC1.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        // Fails
        {
            let input = "Rem_LoCAA.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_none());
        }

        {
            let input = "Rem_LoC.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_none());
        }
    }

    #[test]
    fn test_wildcard_matches_ver() {
        // the following are valid version numbers: 1.0, 1.2.3a, 1, 1a, 1_3a, 77g
        let pattern = "ADN-GDRv<VER>.esp".to_lowercase();

        {
            let input = "ADN-GDRv1.0.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        {
            let input = "ADN-GDRv1.2.3a.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        {
            let input = "ADN-GDRv1.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        {
            let input = "ADN-GDRv1a.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        {
            let input = "ADN-GDRv1_3a.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        {
            let input = "ADN-GDRv77g.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_some());
        }

        // Fails
        {
            let input = "ADN-GDRv1.0_comment.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_none());
        }

        {
            let input = "ADN-GDRv.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_none());
        }

        {
            let input = "ADN-GDRvMyE3.esp".to_lowercase();
            assert!(wild_contains(&[input], &pattern).is_none());
        }
    }

    #[test]
    fn test_redate_mods() {
        let dir = std::env::temp_dir().join(format!("plox_redate_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let names = [
            "morrowind.esm",
            "tribunal.esm",
            "bloodmoon.esm",
            "a.esp",
            "b.esp",
        ];
        let files: Vec<PathBuf> = names.iter().map(|n| dir.join(n)).collect();
        for f in &files {
            File::create(f).unwrap();
        }

        redate_mods(&files).unwrap();

        let secs = |p: &PathBuf| {
            fs::metadata(p)
                .unwrap()
                .modified()
                .unwrap()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        };
        assert_eq!(secs(&files[0]), 1_024_695_106);
        assert_eq!(secs(&files[2]), 1_051_807_050);
        assert_eq!(secs(&files[3]), 1_051_807_110);
        assert_eq!(secs(&files[4]), 1_051_807_170);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_match_filename_version() {
        let inputs = [
            ("a.esp", None),
            ("a_2.0.esp", Some("2.0".to_owned())),
            ("a_3.0_comment.esp", Some("3.0".to_owned())),
            ("a_4.0a_comment.archive", Some("4.0a".to_owned())),
            ("a_5-0-3abc_comment.omwaddon", Some("5-0-3a".to_owned())),
            ("a_6_0_3abc_comment.omwaddon", Some("6_0_3a".to_owned())),
            ("a_7.0_3abc_comment.omwaddon", Some("7.0_3a".to_owned())),
            ("a_7.0-3abc_comment.omwaddon", Some("7.0-3a".to_owned())),
            // TODO this is a valid version number but we don't support it
            ("a_1.1-nightly_comment.esp", Some("1.1".to_owned())),
        ];

        for (input, expected) in &inputs {
            let got = match_filename_version(input);
            assert_eq!(got, *expected);
        }
    }

    #[test]
    fn test_match_desc_version() {
        let inputs = [
            ("a version 1.0", Some("1.0".to_owned())),
            ("a version 1.0_comment", Some("1.0".to_owned())),
            ("a version 1.0", Some("1.0".to_owned())),
            ("some comments", None),
            (
                "some comment about a plugin with version 2.0 and some other stuff",
                Some("2.0".to_owned()),
            ),
            ("do we support v2.4 here? yes", Some("2.4".to_owned())),
            ("many v3.0 anf v2.0 in the header", Some("3.0".to_owned())),
            ("we also match ver6.8 etc", Some("6.8".to_owned())),
            (
                "and other delims v3-5-7-8-8 are supported",
                Some("3-5-7-8-8".to_owned()),
            ),
            (
                "and other delims v1_5_5 are supported",
                Some("1_5_5".to_owned()),
            ),
            ("and mixed ver3.6_7-8 versions", Some("3.6_7-8".to_owned())),
            (
                "and with chars version 9.3a at the end",
                Some("9.3a".to_owned()),
            ),
            (
                "but not actually v3.5.6-nightly semver",
                Some("3.5.6".to_owned()),
            ),
            ("we only have v1.1-5n this stuff", Some("1.1-5n".to_owned())),
        ];

        for (input, expected) in &inputs {
            let got = match_desc_version(input);
            assert_eq!(got, *expected);
        }
    }

    #[test]
    fn test_get_semver() {
        let inputs = [
            ("nothing", None),
            ("1.0", Some(Version::new(1, 0, 0))),
            ("1_5_5", Some(Version::new(1, 5, 5))),
            ("3.6_7", Some(Version::new(3, 6, 7))),
            ("3-6_7", Some(Version::new(3, 6, 7))),
            (
                "9.3a",
                Some(Version {
                    major: 9,
                    minor: 0,
                    patch: 0,
                    pre: Prerelease::new("3a").unwrap(),
                    build: BuildMetadata::EMPTY,
                }),
            ),
            (
                "1.1-5n",
                Some(Version {
                    major: 1,
                    minor: 1,
                    patch: 0,
                    pre: Prerelease::new("5n").unwrap(),
                    build: BuildMetadata::EMPTY,
                }),
            ),
        ];

        for (input, expected) in &inputs {
            let got = get_semver(input.to_owned());
            assert_eq!(got, *expected);
        }
    }
}
