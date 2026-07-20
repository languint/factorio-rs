//! `factorio-rs add <path>` - add a factorio-rs library via Cargo and Factorio.toml.

use std::path::{Component, Path, PathBuf};

use toml_edit::{Array, DocumentMut, Formatted, InlineTable, Item, Value};

use crate::{
    api_crate::load_library_exports,
    cargo_manifest::CargoPackage,
    error::{CliError, CliResult, project_root},
};

/// Result of adding a library dependency; used for CLI messaging and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddResult {
    pub crate_name: String,
    pub rust_crate: String,
    pub dep_path: PathBuf,
    pub cargo_dep_added: bool,
    pub factorio_deps_added: Vec<String>,
    pub remote_fns: Vec<String>,
}

/// Add `lib_path` (a factorio-rs library project) as a normal Cargo dependency.
///
/// Expects the library to have been built once so `[package.metadata.factorio]`
/// (and crate-root re-exports) exist. Prefer `cargo add --path` equivalently for
/// the Cargo.toml half.
///
/// # Errors
/// Returns when the library project, its export metadata, or TOML edits fail.
pub fn add(consumer_root: &Path, lib_path: &Path) -> CliResult<AddResult> {
    let lib_root = resolve_lib_root(lib_path)?;
    let _package = CargoPackage::load(&lib_root)?;
    let manifest = load_library_exports(&lib_root)?;

    let lib_rel = relative_path(consumer_root, &lib_root)?;
    let package_name = manifest.mod_name.replace('_', "-");
    let rust_crate = package_name.replace('-', "_");
    let remote_fns: Vec<String> = manifest
        .remotes
        .iter()
        .map(|remote| remote.function.clone())
        .collect();

    let cargo_dep_added = ensure_cargo_dependency(consumer_root, &package_name, &lib_rel)?;
    let factorio_deps_added =
        ensure_factorio_dependencies(consumer_root, std::slice::from_ref(&manifest.dependency))?;

    Ok(AddResult {
        crate_name: package_name,
        rust_crate,
        dep_path: lib_rel,
        cargo_dep_added,
        factorio_deps_added,
        remote_fns,
    })
}

fn resolve_lib_root(lib_path: &Path) -> CliResult<PathBuf> {
    let root = project_root(Some(lib_path))?;
    let config_path = root.join("Factorio.toml");
    if !config_path.exists() {
        return Err(CliError::NotFound { path: config_path });
    }
    Ok(root)
}

fn ensure_cargo_dependency(
    consumer_root: &Path,
    package_name: &str,
    lib_rel: &Path,
) -> CliResult<bool> {
    let manifest_path = consumer_root.join("Cargo.toml");
    let contents =
        std::fs::read_to_string(&manifest_path).map_err(|source| CliError::ReadFile {
            path: manifest_path.clone(),
            source,
        })?;
    let mut doc = contents
        .parse::<DocumentMut>()
        .map_err(|source| CliError::TomlEdit {
            path: manifest_path.clone(),
            message: source.to_string(),
        })?;

    let deps = doc
        .entry("dependencies")
        .or_insert(Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| CliError::TomlEdit {
            path: manifest_path.clone(),
            message: "`[dependencies]` must be a table".to_string(),
        })?;

    // Drop legacy stub / api path deps from earlier factorio-rs versions.
    deps.remove(format!("{package_name}-api").as_str());
    if let Some(existing) = deps.get(package_name)
        && let Some(table) = existing.as_inline_table()
        && let Some(path) = table.get("path").and_then(Value::as_str)
        && (path.contains("target/factorio-rs/bindings") || path.contains(".factorio-rs/bindings"))
    {
        deps.remove(package_name);
    }

    let desired = path_to_toml_string(lib_rel);
    if let Some(existing) = deps.get(package_name)
        && let Some(table) = existing.as_inline_table()
        && table
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| path == desired)
    {
        return Ok(false);
    }

    let mut table = InlineTable::new();
    table.insert("path", Value::String(Formatted::new(desired)));
    deps.insert(package_name, Item::Value(Value::InlineTable(table)));

    std::fs::write(&manifest_path, doc.to_string()).map_err(|source| CliError::WriteFile {
        path: manifest_path,
        source,
    })?;
    Ok(true)
}

fn ensure_factorio_dependencies(
    consumer_root: &Path,
    deps_to_add: &[String],
) -> CliResult<Vec<String>> {
    if deps_to_add.is_empty() {
        return Ok(Vec::new());
    }

    let config_path = consumer_root.join("Factorio.toml");
    let contents = std::fs::read_to_string(&config_path).map_err(|source| CliError::ReadFile {
        path: config_path.clone(),
        source,
    })?;
    let mut doc = contents
        .parse::<DocumentMut>()
        .map_err(|source| CliError::TomlEdit {
            path: config_path.clone(),
            message: source.to_string(),
        })?;

    let mod_table = doc
        .entry("mod")
        .or_insert(Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| CliError::TomlEdit {
            path: config_path.clone(),
            message: "`[mod]` must be a table".to_string(),
        })?;

    let existing = match mod_table.get("dependencies") {
        Some(Item::Value(Value::Array(array))) => array
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    let mut added = Vec::new();
    let mut array = Array::new();
    for dep in &existing {
        array.push(dep.as_str());
    }
    for dep in deps_to_add {
        if existing.iter().any(|have| have == dep) || added.iter().any(|have| have == dep) {
            continue;
        }
        array.push(dep.as_str());
        added.push(dep.clone());
    }

    if added.is_empty() {
        return Ok(added);
    }

    mod_table.insert("dependencies", Item::Value(Value::Array(array)));
    std::fs::write(&config_path, doc.to_string()).map_err(|source| CliError::WriteFile {
        path: config_path,
        source,
    })?;
    Ok(added)
}

fn relative_path(from_dir: &Path, to: &Path) -> CliResult<PathBuf> {
    let from = std::fs::canonicalize(from_dir).map_err(|source| CliError::ReadDir {
        path: from_dir.to_path_buf(),
        source,
    })?;
    let to = std::fs::canonicalize(to).map_err(|source| CliError::ReadDir {
        path: to.to_path_buf(),
        source,
    })?;

    let mut from_components = from.components().peekable();
    let mut to_components = to.components().peekable();

    while let (Some(a), Some(b)) = (from_components.peek(), to_components.peek()) {
        if a == b {
            from_components.next();
            to_components.next();
        } else {
            break;
        }
    }

    let mut result = PathBuf::new();
    for _ in from_components {
        result.push("..");
    }
    for component in to_components {
        match component {
            Component::RootDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    Ok(result)
}

fn path_to_toml_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::{api_crate::publish_exports, manifest::RemoteExport};

    #[test]
    fn add_wires_cargo_path_to_library() {
        let temp = tempfile::TempDir::new().unwrap();
        let consumer = temp.path().join("consumer");
        let provider = temp.path().join("provider");
        std::fs::create_dir_all(consumer.join("src")).unwrap();
        std::fs::create_dir_all(provider.join("src")).unwrap();

        std::fs::write(
            consumer.join("Cargo.toml"),
            r#"[package]
name = "consumer"
version = "0.1.0"
edition = "2024"

[dependencies]
factorio-rs = "0.1"
"#,
        )
        .unwrap();
        std::fs::write(
            consumer.join("Factorio.toml"),
            r#"source = "src"
output_dir = "dist"

[mod]
title = "consumer"
factorio_version = "2.0"
"#,
        )
        .unwrap();

        std::fs::write(
            provider.join("Factorio.toml"),
            r#"source = "src"
output_dir = "dist"

[mod]
title = "provider"
factorio_version = "2.0"
"#,
        )
        .unwrap();
        std::fs::write(
            provider.join("Cargo.toml"),
            r#"[package]
name = "provider"
version = "0.2.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#,
        )
        .unwrap();
        std::fs::write(provider.join("src/lib.rs"), "pub mod control {}\n").unwrap();

        let package = CargoPackage {
            name: "provider".to_string(),
            version: "0.2.0".to_string(),
            authors: None,
        };
        publish_exports(
            &provider,
            &package,
            &[RemoteExport {
                module: "control".to_string(),
                function: "greet".to_string(),
                interface: "provider".to_string(),
                params: vec![("name".to_string(), Some("&str".to_string()))],
            }],
            &[],
            &[],
        )
        .unwrap();

        let result = add(&consumer, &provider).unwrap();
        assert!(result.cargo_dep_added);
        assert_eq!(result.factorio_deps_added, vec!["provider >= 0.2.0"]);
        assert_eq!(result.rust_crate, "provider");
        assert_eq!(result.dep_path, PathBuf::from("../provider"));

        let cargo = std::fs::read_to_string(consumer.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("provider ="));
        assert!(cargo.contains("path = \"../provider\""));
        assert!(!cargo.contains("bindings"));
        assert!(!cargo.contains("provider-api"));

        let again = add(&consumer, &provider).unwrap();
        assert!(!again.cargo_dep_added);
    }
}
