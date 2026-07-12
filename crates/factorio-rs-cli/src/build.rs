use std::path::{Path, PathBuf};

use factorio_codegen::LuaGenerator;
use factorio_frontend::{
    FrontendError, ParseOptions, discover_modules, lua_output_path,
    parse_discovered_module_with_options,
};
use factorio_ir::{lint::Diagnostic, module::Module, prune::prune_modules};

use crate::{
    assets,
    cargo_manifest::CargoPackage,
    config::Config,
    error::{CliError, CliResult},
    locale,
    manifest::{
        StageModules, collect_event_registrations, collect_stage_module, write_mod_manifests,
    },
};

/// Options that select how a project is transpiled.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Profile name from `Factorio.toml` (`debug`, `release`, or custom).
    pub profile: String,
    /// Optional CLI override for the profile's debug comment level.
    pub debug_level: Option<u8>,
}

impl BuildOptions {
    #[must_use]
    pub fn new(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            debug_level: None,
        }
    }

    #[must_use]
    pub const fn with_debug_level(mut self, debug_level: Option<u8>) -> Self {
        self.debug_level = debug_level;
        self
    }
}

/// Transpile Rust sources to a loadable Factorio mod directory.
pub fn build(project_root: &Path, options: &BuildOptions) -> CliResult<Vec<PathBuf>> {
    let config = Config::load(project_root)?;
    let mut profile = config.resolve_profile(&options.profile);
    if let Some(level) = options.debug_level {
        profile.debug_level = Some(level);
    }

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

    let lint_config = config.lints.resolve()?;
    let mut lint_diagnostics = Vec::<Diagnostic>::new();

    let mut outputs = Vec::new();
    let mut event_registrations = Vec::new();
    let mut stage_modules = StageModules::new();
    let mut discovered_modules = Vec::new();

    for source_path in sources {
        let source = std::fs::read_to_string(&source_path).map_err(|err| CliError::ReadFile {
            path: source_path.clone(),
            source: err,
        })?;
        let discovered = discover_modules(&source_dir, &source_path, &source)?;

        let lua_module_prefix_early = config.emit.lua_module_prefix.as_deref().unwrap_or("");
        let parse_options =
            ParseOptions::new(&lint_config).with_prefix(lua_module_prefix_early);
        for module_spec in discovered {
            let module = parse_discovered_module_with_options(
                &module_spec,
                &parse_options,
                &mut lint_diagnostics,
            )
            .map_err(|err| match err {
                FrontendError::Lint(diagnostic) => CliError::Lint(diagnostic),
                other => CliError::Frontend(other),
            })?;
            discovered_modules.push((module_spec, module));
        }
    }

    for diagnostic in &lint_diagnostics {
        eprintln!("warning: {diagnostic}");
    }

    if discovered_modules.is_empty() {
        return Err(CliError::NoSourceFiles { path: source_dir });
    }

    if profile.prune_dead_code {
        let mut modules = discovered_modules
            .iter()
            .map(|(_, module)| module.clone())
            .collect::<Vec<_>>();
        prune_modules(&mut modules);
        for ((_, module), pruned) in discovered_modules.iter_mut().zip(modules) {
            *module = pruned;
        }
    }

    let lua_module_prefix = config.emit.lua_module_prefix.as_deref().unwrap_or("");
    for (module_spec, module) in &discovered_modules {
        let output_path = transpile_module(
            module_spec,
            module,
            &lua_dir,
            &package.name,
            profile.debug_level,
            lua_module_prefix,
            &profile.name,
        )?;
        event_registrations.extend(collect_event_registrations(module));
        if module.stage.has_side_effect_entry()
            && let Some(stage_module) = collect_stage_module(module, module.stage)
        {
            stage_modules.push(module.stage, stage_module);
        }
        outputs.push(output_path);
    }

    write_mod_manifests(
        &output_dir,
        &package,
        &config,
        &event_registrations,
        &stage_modules,
        &profile.name,
    )?;
    outputs.push(output_dir.join("control.lua"));
    outputs.push(output_dir.join("info.json"));

    let locale_files: Vec<_> = discovered_modules
        .iter()
        .flat_map(|(_, module)| module.locales.iter().cloned())
        .collect();
    outputs.extend(locale::write_locale_files(&output_dir, &locale_files)?);

    if let Some(thumbnail) = assets::copy_thumbnail(project_root, &output_dir, &config)? {
        outputs.push(thumbnail);
    }

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

/// Prepend `prefix` to the last dotted segment of `module_name`.
/// `("settings", "ms")` -> `"ms_settings"`;
/// `("shared.util", "ms")` -> `"shared.ms_util"`.
fn prefix_module_name(module_name: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        return module_name.to_string();
    }
    module_name.rfind('.').map_or_else(
        || format!("{prefix}_{module_name}"),
        |dot| {
            format!(
                "{}.{prefix}_{}",
                &module_name[..dot],
                &module_name[dot + 1..]
            )
        },
    )
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
    module_prefix: &str,
    profile: &str,
) -> CliResult<PathBuf> {
    let mut generator = debug_level.map_or_else(
        || LuaGenerator::with_mod_name(mod_name),
        |level| LuaGenerator::with_mod_name_and_debug(mod_name, level),
    );
    if !module_prefix.is_empty() {
        generator = generator.with_module_prefix(module_prefix);
    }
    generator = generator.with_profile(profile);
    let lua = generator.generate_module(module)?;

    // Apply prefix to the last segment of the module name for the output file path.
    let prefixed_module_name = prefix_module_name(&discovered.module_name, module_prefix);
    let output_path = lua_output_path(lua_dir, &prefixed_module_name);
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
                "legacy".to_string(), // unknown name -> Stage::Shared
                "shared.player".to_string(),
                "shared.player.health".to_string(),
            ]
        );
    }
}
