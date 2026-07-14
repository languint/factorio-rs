//! Run rustc typechecking against Factorio API stubs via Cargo.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::{
    api_crate,
    error::{CliError, CliResult},
};

/// Typecheck the project with `cargo check` (Factorio API stubs + deps).
///
/// Stdout/stderr are inherited so rustc diagnostics look normal.
///
/// # Errors
/// Returns [`CliError::TypecheckFailed`] when `cargo check` exits non-zero, or
/// [`CliError::CargoMetadata`] if cargo cannot be spawned.
pub fn cargo_check(project_root: &Path) -> CliResult<()> {
    // Dependents (and the library itself) need `factorio_exports.rs` for root remotes.
    api_crate::ensure_factorio_exports(project_root)?;
    // Path deps: refresh their re-exports from Cargo metadata before rustc runs.
    refresh_path_dep_exports(project_root)?;

    let manifest = project_root.join("Cargo.toml");
    let status = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--message-format=human")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|source| CliError::CargoMetadata {
            message: format!("failed to run `cargo check`: {source}"),
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(CliError::TypecheckFailed)
    }
}

fn refresh_path_dep_exports(project_root: &Path) -> CliResult<()> {
    let manifest = project_root.join("Cargo.toml");
    if !manifest.exists() {
        return Ok(());
    }
    let contents = std::fs::read_to_string(&manifest).map_err(|source| CliError::ReadFile {
        path: manifest.clone(),
        source,
    })?;
    let value: toml::Value =
        toml::from_str(&contents).map_err(|source| CliError::CargoManifestParse {
            path: manifest.clone(),
            source,
        })?;
    let Some(deps) = value.get("dependencies").and_then(toml::Value::as_table) else {
        return Ok(());
    };
    for dep in deps.values() {
        let Some(path) = dep.get("path").and_then(toml::Value::as_str) else {
            continue;
        };
        let dep_root = project_root.join(path);
        if dep_root.join("Factorio.toml").exists() || dep_root.join("Cargo.toml").exists() {
            api_crate::ensure_factorio_exports(&dep_root)?;
        }
    }
    Ok(())
}
