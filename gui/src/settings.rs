use std::path::Path;

use log::error;

pub const PLOX_LOG_FILE: &str = "plox.log";
pub const PLOX_CONF_FILE: &str = "plox.toml";

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

    pub fn save(&self, path: &Path) {
        match toml::to_string_pretty(self) {
            Ok(s) => {
                if let Err(e) = std::fs::write(path, s) {
                    error!("Error writing settings to file: {e}");
                }
            }
            Err(e) => error!("Error serializing settings: {e}"),
        }
    }
}
