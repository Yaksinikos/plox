use std::path::{Path, PathBuf};

use log::error;

pub const PLOX_LOG_FILE: &str = "plox.log";
pub const PLOX_CONF_FILE: &str = "plox.toml";

/// The `plox.toml` in the current folder if it exists, otherwise the one in the user config folder
#[must_use]
pub fn settings_file() -> PathBuf {
    plox::local_or_platform(PLOX_CONF_FILE, plox::user_config_dir())
}

/// The log file, next to the settings file
#[must_use]
pub fn log_file() -> PathBuf {
    settings_file().with_file_name(PLOX_LOG_FILE)
}

const SETTINGS_KEYS: [&str; 6] = [
    "game",
    "no_rules_download",
    "log_level",
    "log_to_file",
    "ignore_warnings",
    "backup_load_order",
];

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "mirrors independent plox.toml switches"
)]
pub struct AppSettings {
    /// Specifies the game to use
    pub game: Option<plox::ESupportedGame>,

    /// set to not download rules
    pub no_rules_download: bool,

    /// log level
    pub log_level: Option<String>,

    /// use a log file bool
    pub log_to_file: bool,

    /// ignore warnings
    pub ignore_warnings: bool,

    /// back up the load order file (Morrowind.ini or openmw.cfg) once per day before saving
    pub backup_load_order: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            game: None,
            no_rules_download: false,
            log_level: None,
            log_to_file: false,
            ignore_warnings: false,
            backup_load_order: true,
        }
    }
}

impl AppSettings {
    #[must_use]
    pub fn from_file(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => match toml::from_str(&s) {
                Ok(s) => s,
                Err(e) => {
                    error!("Error deserializing settings: {e}");
                    Self::default()
                }
            },
            Err(e) => {
                error!("Error reading settings file: {e}");
                Self::default()
            }
        }
    }

    /// Saves the settings, keeping unrelated sections (e.g. `[renderer]`) of the file.
    pub fn save(&self, path: &Path) {
        match toml::Table::try_from(self) {
            Ok(table) => {
                let entries: Vec<(&str, toml::Value)> =
                    table.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                crate::user_config::merge_into_file(path, &SETTINGS_KEYS, &entries);
            }
            Err(e) => error!("Error serializing settings: {e}"),
        }
    }
}
