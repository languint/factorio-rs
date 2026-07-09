use std::path::{Path, PathBuf};

use factorio_codegen::LuaGenerator;
use factorio_frontend::{discover_modules, lua_output_path, parse_discovered_module};
use factorio_ir::{module::Module, prune::prune_modules};

use crate::{
    cargo_manifest::CargoPackage,
    config::Config,
    error::{CliError, CliResult},
    manifest::{collect_event_registrations, write_mod_manifests},
};

/// Transpile Rust sources in a project to a loadable Factorio mod directory.
pub fn build(project_root: &Path, debug_level: Option<u8>) -> CliResult<Vec<PathBuf>> {
    let config = Config::load(project_root)?;
    let package = CargoPackage::load(project_root)?;
    let source_dir = project_root.join(&config.source);
    let output_dir = project_root.join(&config.output_dir);
    let lua_dir = output_dir.join("lua");

    if !source_dir.is_dir() {
        return Err(CliError::NotFound { path: source_dir });
    }

    let sources = collect_rust_sources(&source_dir)?;
    if sources.is_empty() {
        return Err(CliError::NoSourceFiles { path: source_dir });
    }

    purge_output_dir(&output_dir)?;
    std::fs::create_dir_all(&lua_dir).map_err(|source| CliError::CreateDir {
        path: lua_dir.clone(),
        source,
    })?;

    let mut outputs = Vec::new();
    let mut event_registrations = Vec::new();
    let mut discovered_modules = Vec::new();

    for source_path in sources {
        let source = std::fs::read_to_string(&source_path).map_err(|err| CliError::ReadFile {
            path: source_path.clone(),
            source: err,
        })?;
        let discovered = discover_modules(&source_dir, &source_path, &source)?;

        for module_spec in discovered {
            let module = parse_discovered_module(&module_spec)?;
            discovered_modules.push((module_spec, module));
        }
    }

    if discovered_modules.is_empty() {
        return Err(CliError::NoSourceFiles { path: source_dir });
    }

    if config.prune_dead_code {
        let mut modules = discovered_modules
            .iter()
            .map(|(_, module)| module.clone())
            .collect::<Vec<_>>();
        prune_modules(&mut modules);
        for ((_, module), pruned) in discovered_modules.iter_mut().zip(modules) {
            *module = pruned;
        }
    }

    for (module_spec, module) in discovered_modules {
        let output_path =
            transpile_module(&module_spec, &module, &lua_dir, &package.name, debug_level)?;
        event_registrations.extend(collect_event_registrations(&module));
        outputs.push(output_path);
    }

    write_mod_manifests(&output_dir, &package, &config, &event_registrations)?;
    outputs.push(output_dir.join("control.lua"));
    outputs.push(output_dir.join("info.json"));

    Ok(outputs)
}

fn purge_output_dir(output_dir: &Path) -> CliResult<()> {
    if !output_dir.exists() {
        return Ok(());
    }

    std::fs::remove_dir_all(output_dir).map_err(|source| CliError::RemoveDir {
        path: output_dir.to_path_buf(),
        source,
    })
}

fn collect_rust_sources(source_dir: &Path) -> CliResult<Vec<PathBuf>> {
    let mut sources = Vec::new();
    collect_rust_sources_recursive(source_dir, &mut sources)?;
    sources.sort();
    Ok(sources)
}

fn collect_rust_sources_recursive(current_dir: &Path, sources: &mut Vec<PathBuf>) -> CliResult<()> {
    for entry in std::fs::read_dir(current_dir).map_err(|source| CliError::ReadDir {
        path: current_dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| CliError::ReadDir {
            path: current_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if path.is_dir() {
            collect_rust_sources_recursive(&path, sources)?;
            continue;
        }

        if is_rust_source(&path) {
            sources.push(path);
        }
    }

    Ok(())
}

fn is_rust_source(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}

fn transpile_module(
    discovered: &factorio_frontend::DiscoveredModule,
    module: &Module,
    lua_dir: &Path,
    mod_name: &str,
    debug_level: Option<u8>,
) -> CliResult<PathBuf> {
    let mut generator = debug_level.map_or_else(
        || LuaGenerator::with_mod_name(mod_name),
        |level| LuaGenerator::with_mod_name_and_debug(mod_name, level),
    );
    let lua = generator.generate_module(module)?;

    let output_path = lua_output_path(lua_dir, &discovered.module_name);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    std::fs::write(&output_path, lua).map_err(|source| CliError::WriteFile {
        path: output_path.clone(),
        source,
    })?;

    Ok(output_path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn purge_output_dir_removes_stale_generated_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_dir = temp_dir.path().join("dist");
        std::fs::create_dir_all(output_dir.join("lua/player")).unwrap();
        std::fs::write(output_dir.join("stale.lua"), "stale").unwrap();
        std::fs::write(output_dir.join("lua/player/old.lua"), "old").unwrap();

        purge_output_dir(&output_dir).unwrap();

        assert!(!output_dir.exists());
    }

    use factorio_frontend::discover_modules;

    #[test]
    fn discovers_path_based_and_attribute_based_sources() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("lib.rs"),
            r"
            #[factorio_rs::control]
            mod control {
                pub fn on_init() {}
            }
        ",
        )
        .unwrap();
        std::fs::create_dir_all(source_dir.join("shared/player")).unwrap();
        std::fs::write(source_dir.join("shared/player.rs"), "mod health;").unwrap();
        std::fs::write(source_dir.join("shared/player/health.rs"), "pub fn f() {}").unwrap();
        std::fs::write(source_dir.join("legacy.rs"), "pub fn legacy() {}").unwrap();

        let sources = collect_rust_sources(&source_dir).unwrap();
        let mut module_names = Vec::new();
        for path in sources {
            let source = std::fs::read_to_string(&path).unwrap();
            for module in discover_modules(&source_dir, &path, &source).unwrap() {
                module_names.push(module.module_name);
            }
        }
        module_names.sort();

        assert_eq!(
            module_names,
            vec![
                "control".to_string(),
                "shared.player".to_string(),
                "shared.player.health".to_string(),
            ]
        );
    }
}
