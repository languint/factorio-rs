use std::path::Path;

use serde::Deserialize;

use crate::error::{CliError, CliResult};

#[derive(Debug, Deserialize)]
pub struct CargoManifest {
    pub package: CargoPackage,
}

#[derive(Debug, Deserialize)]
pub struct CargoPackage {
    pub name: String,
    pub version: String,
    pub authors: Option<Vec<String>>,
}

impl CargoPackage {
    pub fn load(project_root: &Path) -> CliResult<Self> {
        let manifest_path = project_root.join("Cargo.toml");
        let contents =
            std::fs::read_to_string(&manifest_path).map_err(|source| CliError::ReadFile {
                path: manifest_path.clone(),
                source,
            })?;

        let manifest: CargoManifest = toml::from_str(&contents).map_err(|source| {
            CliError::CargoManifestParse {
                path: manifest_path,
                source,
            }
        })?;

        Ok(manifest.package)
    }

    pub fn author_label(&self) -> String {
        self.authors
            .as_ref()
            .and_then(|authors| authors.first())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
    }
}
