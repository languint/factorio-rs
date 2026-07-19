use std::path::Path;

use crate::error::{CliError, CliResult};

#[derive(Debug, Clone)]
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

        let value: toml::Value =
            toml::from_str(&contents).map_err(|source| CliError::CargoManifestParse {
                path: manifest_path.clone(),
                source,
            })?;

        let package = value
            .get("package")
            .ok_or_else(|| CliError::CargoMetadata {
                message: format!("`{}` missing [package]", manifest_path.display()),
            })?;

        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| CliError::CargoMetadata {
                message: format!("`{}` missing [package].name", manifest_path.display()),
            })?
            .to_string();

        let version = resolve_package_version(project_root, package, &manifest_path)?;

        let authors = package.get("authors").and_then(|authors| {
            authors.as_array().map(|array| {
                array
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
        });

        Ok(Self {
            name,
            version,
            authors,
        })
    }

    pub fn author_label(&self) -> String {
        self.authors
            .as_ref()
            .and_then(|authors| authors.first())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Resolve `[package].version`, including `version.workspace = true`.
pub fn resolve_package_version(
    project_root: &Path,
    package: &toml::Value,
    manifest_path: &Path,
) -> CliResult<String> {
    match package.get("version") {
        Some(toml::Value::String(version)) => Ok(version.clone()),
        Some(toml::Value::Table(table))
            if table
                .get("workspace")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false) =>
        {
            resolve_workspace_package_field(project_root, "version").ok_or_else(|| {
                CliError::CargoMetadata {
                    message: format!(
                        "`{}` has `version.workspace = true`, but no \
                         `[workspace.package].version` was found in a parent Cargo.toml",
                        manifest_path.display()
                    ),
                }
            })
        }
        Some(other) => Err(CliError::CargoMetadata {
            message: format!(
                "`{}` has unsupported [package].version form: {other}",
                manifest_path.display()
            ),
        }),
        None => Err(CliError::CargoMetadata {
            message: format!("`{}` missing [package].version", manifest_path.display()),
        }),
    }
}

fn resolve_workspace_package_field(start: &Path, field: &str) -> Option<String> {
    // Relative paths like `examples/provider` must be absolutized so parent walks
    // reach the workspace root (`.` / cwd), not an empty path after two pops.
    let mut dir = std::path::absolute(start).ok()?;
    if dir.is_file() {
        dir.pop();
    }

    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.is_file()
            && let Ok(contents) = std::fs::read_to_string(&candidate)
            && let Ok(value) = toml::from_str::<toml::Value>(&contents)
            && let Some(version) = value
                .get("workspace")
                .and_then(|workspace| workspace.get("package"))
                .and_then(|package| package.get(field))
                .and_then(toml::Value::as_str)
        {
            return Some(version.to_string());
        }

        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn loads_explicit_version() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join("Cargo.toml"),
            r#"
            [package]
            name = "demo"
            version = "1.2.3"
            "#,
        );
        let package = CargoPackage::load(dir.path()).unwrap();
        assert_eq!(package.name, "demo");
        assert_eq!(package.version, "1.2.3");
    }

    #[test]
    fn loads_workspace_inherited_version() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join("Cargo.toml"),
            r#"
            [workspace]
            members = ["examples/demo"]
            [workspace.package]
            version = "0.1.8"
            "#,
        );
        let member = dir.path().join("examples/demo");
        write(
            &member.join("Cargo.toml"),
            r#"
            [package]
            name = "demo"
            version.workspace = true
            "#,
        );
        let package = CargoPackage::load(&member).unwrap();
        assert_eq!(package.name, "demo");
        assert_eq!(package.version, "0.1.8");
    }

    #[test]
    fn loads_workspace_version_from_relative_member_path() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join("Cargo.toml"),
            r#"
            [workspace]
            members = ["examples/demo"]
            [workspace.package]
            version = "9.9.9"
            "#,
        );
        let member = dir.path().join("examples/demo");
        write(
            &member.join("Cargo.toml"),
            r#"
            [package]
            name = "demo"
            version.workspace = true
            "#,
        );

        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let package = CargoPackage::load(Path::new("examples/demo"));
        std::env::set_current_dir(previous).unwrap();

        let package = package.unwrap();
        assert_eq!(package.version, "9.9.9");
    }
}
