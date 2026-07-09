use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{CliError, CliResult};

const CONFIG_FILE: &str = "Factorio.toml";

fn default_source() -> String {
    "src".to_string()
}

fn default_output_dir() -> String {
    "dist".to_string()
}

const fn default_prune_dead_code() -> bool {
    true
}

/// Metadata written to generated `info.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModConfig {
    pub title: Option<String>,
    pub description: Option<String>,
    pub factorio_version: Option<String>,
}

impl Default for ModConfig {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            factorio_version: Some("2.0".to_string()),
        }
    }
}

/// Project configuration loaded from `Factorio.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    #[serde(default = "default_source")]
    pub source: String,

    #[serde(default = "default_output_dir")]
    pub output_dir: String,

    /// Remove unreachable functions and exports from generated Lua.
    #[serde(default = "default_prune_dead_code")]
    pub prune_dead_code: bool,

    #[serde(default)]
    pub r#mod: ModConfig,
}

impl Config {
    /// Load configuration from `Factorio.toml` in `project_root`.
    pub fn load(project_root: &Path) -> CliResult<Self> {
        let config_path = project_root.join(CONFIG_FILE);
        let contents =
            std::fs::read_to_string(&config_path).map_err(|source| CliError::ReadFile {
                path: config_path.clone(),
                source,
            })?;

        toml::from_str(&contents).map_err(|source| CliError::ConfigParse {
            path: config_path,
            source,
        })
    }

    pub fn config_path(project_root: &Path) -> PathBuf {
        project_root.join(CONFIG_FILE)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::Config;

    #[test]
    fn parses_defaults() {
        let config: Config = toml::from_str("").unwrap();

        assert_eq!(
            config,
            Config {
                source: "src".to_string(),
                output_dir: "dist".to_string(),
                prune_dead_code: true,
                r#mod: super::ModConfig {
                    title: None,
                    description: None,
                    factorio_version: Some("2.0".to_string()),
                },
            }
        );
    }

    #[test]
    fn parses_prune_dead_code_override() {
        let config: Config = toml::from_str("prune_dead_code = false").unwrap();
        assert!(!config.prune_dead_code);
    }
}
