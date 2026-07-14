use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    config::{
        emit::EmitConfig,
        lints::LintsConfig,
        profile::{ProfileSettings, ResolvedProfile, resolve_profile},
    },
    error::{CliError, CliResult},
};

pub mod emit;
pub mod lints;
pub mod profile;

const CONFIG_FILE: &str = "Factorio.toml";

fn default_source() -> String {
    "src".to_string()
}

fn default_output_dir() -> String {
    "dist".to_string()
}

/// Metadata written to generated `info.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModConfig {
    pub title: Option<String>,
    pub description: Option<String>,
    pub factorio_version: Option<String>,
    pub thumbnail: Option<String>,
    /// Extra Factorio dependency strings for `info.json` (`? mod`, `! conflict`, ...).
    /// Merged with deps from Cargo binding crates; this list wins on duplicate mod names.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Deprecated: ignored. Exports are written to `.factorio-rs/exports.json`.
    #[serde(default = "default_emit_api")]
    pub emit_api: bool,
    /// Deprecated: ignored. Exports publish onto the library Cargo package.
    #[serde(default = "default_api_dir")]
    pub api_dir: String,
}

fn default_emit_api() -> bool {
    false
}

fn default_api_dir() -> String {
    "api".to_string()
}

impl Default for ModConfig {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            factorio_version: Some("2.0".to_string()),
            thumbnail: None,
            dependencies: Vec::new(),
            emit_api: false,
            api_dir: default_api_dir(),
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

    #[serde(default)]
    pub emit: EmitConfig,

    #[serde(default)]
    pub r#mod: ModConfig,

    /// Named transpile profiles (`[profiles.debug]`, `[profiles.release]`, ...).
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileSettings>,

    /// Transpile-time safety lint levels (`[lints] unwrap = "allow"`, ...).
    #[serde(default)]
    pub lints: LintsConfig,
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

    /// Resolve a named profile, applying built-in defaults then TOML overrides.
    #[must_use]
    pub fn resolve_profile(&self, profile_name: &str) -> ResolvedProfile {
        resolve_profile(&self.profiles, profile_name)
    }
}
