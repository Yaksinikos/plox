//! Per-user GUI configuration stored in the platform config folder
//! (`~/.config/plox/`, `%APPDATA%\plox\`, `~/Library/Application Support/plox/`).

use std::path::{Path, PathBuf};

use log::{error, warn};
use serde::{Deserialize, Serialize};

use crate::renderer::RendererChoice;

const CONFIG_FILE: &str = crate::settings::PLOX_CONF_FILE;
const CRASH_MARKER_FILE: &str = "renderer.crash";

/// `[renderer]` section of the user config
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RendererConfig {
    pub backend: RendererChoice,
    pub ask_on_startup: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            backend: RendererChoice::Auto,
            ask_on_startup: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct UserConfigFile {
    renderer: RendererConfig,
}

/// The plox folder inside the platform config dir
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    plox::user_config_dir()
}

#[must_use]
pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join(CONFIG_FILE))
}

impl RendererConfig {
    #[must_use]
    pub fn load() -> Self {
        let Some(path) = config_file() else {
            return Self::default();
        };
        let Ok(s) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str::<UserConfigFile>(&s).map_or_else(
            |e| {
                warn!("Invalid user config {}: {e}", path.display());
                Self::default()
            },
            |c| c.renderer,
        )
    }

    pub fn save(&self) {
        let Some(path) = config_file() else {
            error!("No user config folder available");
            return;
        };
        match toml::Table::try_from(self) {
            Ok(table) => merge_into_file(&path, &[], &[("renderer", toml::Value::Table(table))]),
            Err(e) => error!("Error serializing renderer config: {e}"),
        }
    }
}

/// Removes `remove` and writes `entries` as top-level keys into a TOML file, keeping all other keys.
pub fn merge_into_file(path: &Path, remove: &[&str], entries: &[(&str, toml::Value)]) {
    let mut table = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.parse::<toml::Table>().ok())
        .unwrap_or_default();
    for key in remove {
        table.remove(*key);
    }
    for (key, value) in entries {
        table.insert((*key).to_owned(), value.clone());
    }

    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        error!("Error creating {}: {e}", parent.display());
        return;
    }
    match toml::to_string_pretty(&table) {
        Ok(s) => {
            if let Err(e) = std::fs::write(path, s) {
                error!("Error writing {}: {e}", path.display());
            }
        }
        Err(e) => error!("Error serializing {}: {e}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_renderer_section() {
        let c: UserConfigFile =
            toml::from_str("[renderer]\nbackend = \"wgpu\"\nask_on_startup = false\n").unwrap();
        assert_eq!(c.renderer.backend, RendererChoice::Wgpu);
        assert!(!c.renderer.ask_on_startup);

        let c: UserConfigFile = toml::from_str("game = \"Openmw\"\n").unwrap();
        assert_eq!(c.renderer, RendererConfig::default());
    }

    #[test]
    fn merge_keeps_other_keys() {
        let dir = std::env::temp_dir().join(format!("plox_cfg_{}", std::process::id()));
        let path = dir.join("plox.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "log_level = \"debug\"\ngame = \"Openmw\"\n").unwrap();

        merge_into_file(
            &path,
            &["game"],
            &[(
                "renderer",
                toml::Value::Table(toml::Table::try_from(RendererConfig::default()).unwrap()),
            )],
        );

        let table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(table["log_level"].as_str(), Some("debug"));
        assert!(!table.contains_key("game"));
        assert_eq!(table["renderer"]["backend"].as_str(), Some("auto"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parses_renderer_choice() {
        assert_eq!(RendererChoice::parse(" GLOW "), Some(RendererChoice::Glow));
        assert_eq!(RendererChoice::parse("auto"), Some(RendererChoice::Auto));
        assert_eq!(RendererChoice::parse("vulkan"), None);
    }
}

/// Marker that exists while the wgpu renderer is running. If it is still
/// present on startup, the previous wgpu session did not exit cleanly.
pub mod crash_marker {
    use super::{CRASH_MARKER_FILE, config_dir};

    fn path() -> Option<std::path::PathBuf> {
        config_dir().map(|d| d.join(CRASH_MARKER_FILE))
    }

    #[must_use]
    pub fn exists() -> bool {
        path().is_some_and(|p| p.exists())
    }

    pub fn set() {
        if let Some(p) = path() {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&p, b"wgpu") {
                log::warn!("Could not write crash marker {}: {e}", p.display());
            }
        }
    }

    pub fn clear() {
        if let Some(p) = path()
            && p.exists()
            && let Err(e) = std::fs::remove_file(&p)
        {
            log::warn!("Could not remove crash marker {}: {e}", p.display());
        }
    }
}
