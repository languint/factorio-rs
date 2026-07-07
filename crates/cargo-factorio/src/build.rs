use std::path::{Path, PathBuf};

use factorio_codegen::LuaGenerator;
use factorio_frontend::{lua_output_path, module_name_from_source, parse_module};

use crate::{
    config::Config,
    error::{CliError, CliResult},
};

/// Transpile Rust sources in a project to Lua.
pub fn build(project_root: &Path) -> CliResult<Vec<PathBuf>> {
    let config = Config::load(project_root)?;
    let source_dir = project_root.join(&config.source);
    let output_dir = project_root.join(&config.output_dir);

    if !source_dir.is_dir() {
        return Err(CliError::NotFound { path: source_dir });
    }

    let sources = collect_source_files(&source_dir)?;
    if sources.is_empty() {
        return Err(CliError::NoSourceFiles { path: source_dir });
    }

    purge_output_dir(&output_dir)?;
    std::fs::create_dir_all(&output_dir).map_err(|source| CliError::CreateDir {
        path: output_dir.clone(),
        source,
    })?;

    let mut outputs = Vec::new();
    for source_path in sources {
        outputs.push(transpile_file(&source_path, &source_dir, &output_dir)?);
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

fn collect_source_files(source_dir: &Path) -> CliResult<Vec<PathBuf>> {
    let mut sources = Vec::new();
    collect_source_files_recursive(source_dir, source_dir, &mut sources)?;
    sources.sort();
    Ok(sources)
}

fn collect_source_files_recursive(
    source_dir: &Path,
    current_dir: &Path,
    sources: &mut Vec<PathBuf>,
) -> CliResult<()> {
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
            collect_source_files_recursive(source_dir, &path, sources)?;
            continue;
        }

        if !is_rust_source(&path) {
            continue;
        }

        if module_name_from_source(source_dir, &path).is_some() {
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

fn transpile_file(source_path: &Path, source_dir: &Path, output_dir: &Path) -> CliResult<PathBuf> {
    let module_name = module_name_from_source(source_dir, source_path).ok_or_else(|| {
        CliError::InvalidProjectPath {
            path: source_path.to_path_buf(),
        }
    })?;

    let source = std::fs::read_to_string(source_path).map_err(|err| CliError::ReadFile {
        path: source_path.to_path_buf(),
        source: err,
    })?;

    let module = parse_module(&source, &module_name)?;
    let lua = LuaGenerator::new().generate_module(&module)?;

    let output_path = lua_output_path(output_dir, &module_name);
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
    use super::*;

    #[test]
    fn purge_output_dir_removes_stale_generated_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_dir = temp_dir.path().join("lua");
        std::fs::create_dir_all(output_dir.join("player")).unwrap();
        std::fs::write(output_dir.join("stale.lua"), "stale").unwrap();
        std::fs::write(output_dir.join("player/old.lua"), "old").unwrap();

        purge_output_dir(&output_dir).unwrap();

        assert!(!output_dir.exists());
    }

    #[test]
    fn collects_nested_source_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(source_dir.join("player")).unwrap();
        std::fs::write(source_dir.join("on_init.rs"), "pub fn on_init() {}").unwrap();
        std::fs::write(source_dir.join("player.rs"), "mod extra_info;").unwrap();
        std::fs::write(source_dir.join("player/extra_info.rs"), "pub fn f() {}").unwrap();

        let sources = collect_source_files(&source_dir).unwrap();
        let module_names = sources
            .iter()
            .map(|path| module_name_from_source(&source_dir, path).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            module_names,
            vec![
                "on_init".to_string(),
                "player.extra_info".to_string(),
                "player".to_string(),
            ]
        );
    }
}
