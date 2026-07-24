//! Publish `#[factorio_rs::export]` surfaces onto the library's own Cargo package
//! so consumers can `cargo add` / path-depend on the mod crate directly.

use std::{
    collections::BTreeSet,
    fmt::Write as _,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use crate::{
    cargo_manifest::CargoPackage,
    error::{CliError, CliResult},
    manifest::{RemoteExport, SharedConst, SharedExport},
    write_if_changed::write_if_changed,
};

/// Optional catalog kept next to the project (human-readable / tooling).
pub const EXPORTS_MANIFEST_REL: &str = ".factorio-rs/exports.json";
/// Generated crate-root re-exports for Cargo dependents (`use provider::greet`).
pub const API_REEXPORTS_REL: &str = "src/factorio_exports.rs";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportsManifest {
    pub mod_name: String,
    pub version: String,
    pub dependency: String,
    pub module_root: String,
    pub interface: String,
    pub remotes: Vec<ManifestRemote>,
    pub shared_fns: Vec<ManifestSharedFn>,
    pub shared_consts: Vec<ManifestSharedConst>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestRemote {
    pub function: String,
    pub module: String,
    pub interface: String,
    pub params: Vec<ManifestParam>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestSharedFn {
    pub module: String,
    pub function: String,
    pub params: Vec<ManifestParam>,
    /// `#[factorio_rs::inline]` shared hot path (require, not remote).
    #[serde(default)]
    pub inline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestSharedConst {
    pub module: String,
    pub name: String,
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestParam {
    pub name: String,
    pub ty: Option<String>,
}

/// Ensure `src/factorio_exports.rs` is present/fresh before `cargo check`.
///
/// Regenerates from `[package.metadata.factorio]` when present. If the crate roots
/// `mod factorio_exports` but the file is missing (first export build), writes an
/// empty stub so rustc can run.
///
/// # Errors
/// Returns I/O or TOML parse errors.
pub fn ensure_factorio_exports(project_root: &Path) -> CliResult<()> {
    if let Some(manifest) = load_exports_from_cargo_toml(project_root)? {
        let remotes: Vec<RemoteExport> = manifest
            .remotes
            .iter()
            .map(|remote| RemoteExport {
                module: remote.module.clone(),
                function: remote.function.clone(),
                interface: remote.interface.clone(),
                params: remote
                    .params
                    .iter()
                    .map(|param| (param.name.clone(), param.ty.clone()))
                    .collect(),
            })
            .collect();
        write_api_reexports(project_root, &remotes)?;
        let _ = ensure_lib_rs_wires_factorio_exports(project_root)?;
        return Ok(());
    }

    let path = project_root.join(API_REEXPORTS_REL);
    if path.exists() || !lib_rs_references_factorio_exports(project_root)? {
        return Ok(());
    }
    write_api_reexports(project_root, &[])?;
    let _ = ensure_lib_rs_wires_factorio_exports(project_root)?;
    Ok(())
}

fn lib_rs_references_factorio_exports(project_root: &Path) -> CliResult<bool> {
    let lib = project_root.join("src/lib.rs");
    if !lib.exists() {
        return Ok(false);
    }
    let contents =
        std::fs::read_to_string(&lib).map_err(|source| CliError::ReadFile { path: lib, source })?;
    Ok(contents.contains("factorio_exports"))
}

/// Write exports into Cargo metadata + `src/factorio_exports.rs` (+ optional JSON).
///
/// Also wires `mod factorio_exports; pub use factorio_exports::*;` into `src/lib.rs`
/// when missing so dependents can resolve crate-root remotes without manual boilerplate.
///
/// # Errors
/// Returns I/O or TOML edit errors.
pub fn publish_exports(
    project_root: &Path,
    package: &CargoPackage,
    remote_exports: &[RemoteExport],
    shared_exports: &[SharedExport],
    shared_consts: &[SharedConst],
) -> CliResult<Vec<PathBuf>> {
    if remote_exports.is_empty() && shared_exports.is_empty() && shared_consts.is_empty() {
        return Ok(Vec::new());
    }

    let manifest = build_manifest(package, remote_exports, shared_exports, shared_consts);
    let mut outputs = Vec::new();

    outputs.push(write_exports_json(project_root, &manifest)?);
    write_cargo_factorio_metadata(project_root, &manifest)?;
    outputs.push(project_root.join("Cargo.toml"));
    outputs.push(write_api_reexports(project_root, remote_exports)?);
    if ensure_lib_rs_wires_factorio_exports(project_root)? {
        outputs.push(project_root.join("src/lib.rs"));
    }

    Ok(outputs)
}

/// Load export metadata, preferring richer `.factorio-rs/exports.json` when present.
///
/// Cargo metadata only stores remote function *names*; the JSON catalog keeps
/// params and shared exports/consts. Prefer JSON when it has strictly more detail.
///
/// # Errors
/// Returns when neither source is available or parse fails.
pub fn load_library_exports(lib_root: &Path) -> CliResult<ExportsManifest> {
    let cargo = load_exports_from_cargo_toml(lib_root)?;
    let json = match load_exports_manifest(lib_root) {
        Ok(manifest) => Some(manifest),
        Err(CliError::ExportsManifestMissing { .. }) => None,
        Err(err) => return Err(err),
    };
    match (cargo, json) {
        (Some(cargo_manifest), Some(json_manifest))
            if exports_json_is_richer(&json_manifest, &cargo_manifest) =>
        {
            Ok(json_manifest)
        }
        (Some(cargo_manifest), _) => Ok(cargo_manifest),
        (None, Some(json_manifest)) => Ok(json_manifest),
        (None, None) => Err(CliError::ExportsManifestMissing {
            path: lib_root.join(EXPORTS_MANIFEST_REL),
        }),
    }
}

fn exports_json_is_richer(json: &ExportsManifest, cargo: &ExportsManifest) -> bool {
    if json.shared_fns.len() > cargo.shared_fns.len()
        || json.shared_consts.len() > cargo.shared_consts.len()
    {
        return true;
    }
    let json_has_params = json.remotes.iter().any(|remote| !remote.params.is_empty());
    let cargo_lacks_params = cargo.remotes.iter().all(|remote| remote.params.is_empty());
    json_has_params && cargo_lacks_params
}

/// Append `mod factorio_exports; pub use factorio_exports::*;` to `lib.rs` when absent.
///
/// Returns `true` when the file was modified.
fn ensure_lib_rs_wires_factorio_exports(project_root: &Path) -> CliResult<bool> {
    let lib = project_root.join("src/lib.rs");
    if !lib.exists() {
        return Ok(false);
    }
    let contents = std::fs::read_to_string(&lib).map_err(|source| CliError::ReadFile {
        path: lib.clone(),
        source,
    })?;
    if contents.contains("factorio_exports") {
        return Ok(false);
    }
    let mut updated = contents;
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(
        "\n// Generated wiring for Cargo dependents (crate-root remotes).\n\
         mod factorio_exports;\n\
         pub use factorio_exports::*;\n",
    );
    std::fs::write(&lib, updated).map_err(|source| CliError::WriteFile { path: lib, source })?;
    Ok(true)
}

/// Load `.factorio-rs/exports.json`.
///
/// # Errors
/// Missing file -> [`CliError::ExportsManifestMissing`].
pub fn load_exports_manifest(lib_root: &Path) -> CliResult<ExportsManifest> {
    let path = lib_root.join(EXPORTS_MANIFEST_REL);
    if !path.exists() {
        return Err(CliError::ExportsManifestMissing { path });
    }
    let contents = std::fs::read_to_string(&path).map_err(|source| CliError::ReadFile {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| CliError::CargoMetadata {
        message: format!("failed to parse `{}`: {source}", path.display()),
    })
}

fn load_exports_from_cargo_toml(lib_root: &Path) -> CliResult<Option<ExportsManifest>> {
    let path = lib_root.join("Cargo.toml");
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path).map_err(|source| CliError::ReadFile {
        path: path.clone(),
        source,
    })?;
    let value: toml::Value =
        toml::from_str(&contents).map_err(|source| CliError::CargoManifestParse {
            path: path.clone(),
            source,
        })?;
    let Some(factorio) = value
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("factorio"))
    else {
        return Ok(None);
    };
    let package = value
        .get("package")
        .ok_or_else(|| CliError::CargoMetadata {
            message: format!("`{}` missing [package]", path.display()),
        })?;
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let version = crate::cargo_manifest::resolve_package_version(lib_root, package, &path)
        .unwrap_or_else(|_| "0.0.0".to_string());

    let mod_name = factorio
        .get("mod_name")
        .and_then(toml::Value::as_str)
        .unwrap_or(&name)
        .to_string();
    let dependencies = factorio
        .get("dependencies")
        .and_then(toml::Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let dependency = dependencies
        .first()
        .cloned()
        .unwrap_or_else(|| format!("{mod_name} >= {version}"));
    let module_root = factorio
        .get("module_root")
        .and_then(toml::Value::as_str)
        .unwrap_or("lua")
        .to_string();
    let interface = factorio
        .get("interface")
        .and_then(toml::Value::as_str)
        .unwrap_or(&mod_name)
        .to_string();
    let remote_fns: Vec<String> = factorio
        .get("remote_fns")
        .and_then(toml::Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Ok(Some(ExportsManifest {
        mod_name,
        version,
        dependency,
        module_root,
        interface: interface.clone(),
        remotes: remote_fns
            .into_iter()
            .map(|function| ManifestRemote {
                function,
                module: "control".to_string(),
                interface: interface.clone(),
                params: Vec::new(),
            })
            .collect(),
        shared_fns: Vec::new(),
        shared_consts: Vec::new(),
    }))
}

fn write_exports_json(project_root: &Path, manifest: &ExportsManifest) -> CliResult<PathBuf> {
    let path = project_root.join(EXPORTS_MANIFEST_REL);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json =
        serde_json::to_string_pretty(manifest).map_err(|source| CliError::CargoMetadata {
            message: format!("failed to serialize exports manifest: {source}"),
        })?;
    write_if_changed(&path, &format!("{json}\n"))?;
    Ok(path)
}

fn write_cargo_factorio_metadata(project_root: &Path, manifest: &ExportsManifest) -> CliResult<()> {
    let path = project_root.join("Cargo.toml");
    let contents = std::fs::read_to_string(&path).map_err(|source| CliError::ReadFile {
        path: path.clone(),
        source,
    })?;
    let mut doc = contents
        .parse::<DocumentMut>()
        .map_err(|source| CliError::TomlEdit {
            path: path.clone(),
            message: source.to_string(),
        })?;

    let package = doc
        .get_mut("package")
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| CliError::TomlEdit {
            path: path.clone(),
            message: "`[package]` must be a table".to_string(),
        })?;

    let metadata = package
        .entry("metadata")
        .or_insert(Item::Table(Table::new()))
        .as_table_like_mut()
        .ok_or_else(|| CliError::TomlEdit {
            path: path.clone(),
            message: "`[package.metadata]` must be a table".to_string(),
        })?;

    let factorio = metadata
        .entry("factorio")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| CliError::TomlEdit {
            path: path.clone(),
            message: "`[package.metadata.factorio]` must be a table".to_string(),
        })?;

    factorio.insert(
        "mod_name",
        Item::Value(Value::from(manifest.mod_name.as_str())),
    );
    let mut deps = Array::new();
    deps.push(manifest.dependency.as_str());
    factorio.insert("dependencies", Item::Value(Value::Array(deps)));
    factorio.insert(
        "module_root",
        Item::Value(Value::from(manifest.module_root.as_str())),
    );
    factorio.insert(
        "interface",
        Item::Value(Value::from(manifest.interface.as_str())),
    );

    let mut remote_fns = Array::new();
    let mut seen = BTreeSet::new();
    for remote in &manifest.remotes {
        if seen.insert(remote.function.as_str()) {
            remote_fns.push(remote.function.as_str());
        }
    }
    if remote_fns.is_empty() {
        factorio.remove("remote_fns");
    } else {
        factorio.insert("remote_fns", Item::Value(Value::Array(remote_fns)));
    }

    let mut inline_fns = Array::new();
    let mut inline_seen = BTreeSet::new();
    for shared in &manifest.shared_fns {
        if shared.inline && inline_seen.insert(shared.function.as_str()) {
            inline_fns.push(shared.function.as_str());
        }
    }
    if inline_fns.is_empty() {
        factorio.remove("inline_fns");
    } else {
        factorio.insert("inline_fns", Item::Value(Value::Array(inline_fns)));
    }

    let rendered = doc.to_string();
    write_if_changed(&path, &rendered)?;
    Ok(())
}

fn write_api_reexports(project_root: &Path, remote_exports: &[RemoteExport]) -> CliResult<PathBuf> {
    let path = project_root.join(API_REEXPORTS_REL);
    let mut out = String::from(
        "// @generated by factorio-rs. Do not edit.\n\
         // Crate-root re-exports of control `#[factorio_rs::export]` functions for Cargo dependents.\n\n",
    );

    let mut seen = BTreeSet::new();
    for export in remote_exports {
        if !seen.insert(export.function.as_str()) {
            continue;
        }
        let module_path = export.module.replace('.', "::");
        let _ = writeln!(out, "pub use crate::{module_path}::{};", export.function);
    }

    if seen.is_empty() {
        out.push_str("// (no control remotes)\n");
    }

    write_if_changed(&path, &out)?;
    Ok(path)
}

fn build_manifest(
    package: &CargoPackage,
    remote_exports: &[RemoteExport],
    shared_exports: &[SharedExport],
    shared_consts: &[SharedConst],
) -> ExportsManifest {
    let interface = remote_exports
        .first()
        .map_or_else(|| package.name.clone(), |export| export.interface.clone());

    ExportsManifest {
        mod_name: package.name.clone(),
        version: package.version.clone(),
        dependency: format!("{} >= {}", package.name, package.version),
        module_root: "lua".to_string(),
        interface,
        remotes: remote_exports
            .iter()
            .map(|export| ManifestRemote {
                function: export.function.clone(),
                module: export.module.clone(),
                interface: export.interface.clone(),
                params: export
                    .params
                    .iter()
                    .map(|(name, ty)| ManifestParam {
                        name: name.clone(),
                        ty: ty.clone(),
                    })
                    .collect(),
            })
            .collect(),
        shared_fns: shared_exports
            .iter()
            .map(|export| ManifestSharedFn {
                module: export.module.clone(),
                function: export.function.clone(),
                params: export
                    .params
                    .iter()
                    .map(|(name, ty)| ManifestParam {
                        name: name.clone(),
                        ty: ty.clone(),
                    })
                    .collect(),
                inline: export.inline,
            })
            .collect(),
        shared_consts: shared_consts
            .iter()
            .map(|konst| ManifestSharedConst {
                module: konst.module.clone(),
                name: konst.name.clone(),
                source_type: konst.source_type.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn ensure_regenerates_reexports_from_cargo_metadata() {
        let temp = tempfile::TempDir::new().unwrap();
        let provider = temp.path().join("provider");
        std::fs::create_dir_all(provider.join("src")).unwrap();
        std::fs::write(
            provider.join("Cargo.toml"),
            r#"[package]
name = "provider"
version = "0.4.0"
edition = "2024"

[package.metadata.factorio]
mod_name = "provider"
dependencies = ["provider >= 0.4.0"]
module_root = "lua"
interface = "provider"
remote_fns = ["greet"]

[lib]
path = "src/lib.rs"
"#,
        )
        .unwrap();
        std::fs::write(
            provider.join("src/lib.rs"),
            "pub mod control {}\nmod factorio_exports;\npub use factorio_exports::*;\n",
        )
        .unwrap();

        ensure_factorio_exports(&provider).unwrap();
        let reexports = std::fs::read_to_string(provider.join("src/factorio_exports.rs")).unwrap();
        assert!(reexports.contains("pub use crate::control::greet;"));
    }

    #[test]
    fn publish_writes_cargo_metadata_and_reexports() {
        let temp = tempfile::TempDir::new().unwrap();
        let provider = temp.path().join("provider");
        std::fs::create_dir_all(provider.join("src")).unwrap();
        std::fs::write(
            provider.join("Cargo.toml"),
            r#"[package]
name = "provider"
version = "0.4.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#,
        )
        .unwrap();
        std::fs::write(provider.join("src/lib.rs"), "pub mod control {}\n").unwrap();

        let package = CargoPackage {
            name: "provider".to_string(),
            version: "0.4.0".to_string(),
            authors: None,
        };
        let remotes = vec![RemoteExport {
            module: "control".to_string(),
            function: "add".to_string(),
            interface: "provider".to_string(),
            params: vec![
                ("a".to_string(), Some("i32".to_string())),
                ("b".to_string(), Some("i32".to_string())),
            ],
        }];

        let outputs = publish_exports(&provider, &package, &remotes, &[], &[]).unwrap();
        assert!(!outputs.is_empty());

        let cargo = std::fs::read_to_string(provider.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("[package.metadata.factorio]"));
        assert!(cargo.contains("remote_fns"));
        assert!(cargo.contains("\"add\"") || cargo.contains("add"));

        let reexports = std::fs::read_to_string(provider.join("src/factorio_exports.rs")).unwrap();
        assert!(reexports.contains("pub use crate::control::add;"));

        let lib_rs = std::fs::read_to_string(provider.join("src/lib.rs")).unwrap();
        assert!(lib_rs.contains("mod factorio_exports;"));
        assert!(lib_rs.contains("pub use factorio_exports::*;"));

        let loaded = load_library_exports(&provider).unwrap();
        assert_eq!(loaded.mod_name, "provider");
        assert!(loaded.remotes.iter().any(|r| r.function == "add"));
        // Prefer richer exports.json (params) over lossy Cargo remote_fns names.
        let add = loaded.remotes.iter().find(|r| r.function == "add").unwrap();
        assert_eq!(add.params.len(), 2);
    }
}
