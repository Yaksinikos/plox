use std::path::Path;

use log::error;

pub const PLOX_LOG_FILE: &str = "plox.log";
pub const PLOX_CONF_FILE: &str = "plox.toml";

const SETTINGS_KEYS: [&str; 5] = [
    "game",
    "no_rules_download",
    "log_level",
    "log_to_file",
    "ignore_warnings",
];

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
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
