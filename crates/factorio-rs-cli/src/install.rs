use std::path::{Path, PathBuf};

use crate::{
    build::{BuildOptions, build},
    cargo_manifest::CargoPackage,
    config::Config,
    error::{CliError, CliResult},
    paths::factorio_mods_dir,
};

/// Build the mod and copy it into the Factorio mods directory.
pub fn install(project_root: &Path, options: &BuildOptions) -> CliResult<PathBuf> {
    build(project_root, options)?;

    let package = CargoPackage::load(project_root)?;
    let config = Config::load(project_root)?;
    let output_dir = project_root.join(&config.output_dir);
    let mods_dir = factorio_mods_dir()?;
    let dest = mods_dir.join(format!("{}_{}", package.name, package.version));

    copy_dir_recursive(&output_dir, &dest)?;
    Ok(dest)
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> CliResult<()> {
    if dest.exists() {
        std::fs::remove_dir_all(dest).map_err(|source| CliError::RemoveDir {
            path: dest.to_path_buf(),
            source,
        })?;
    }

    std::fs::create_dir_all(dest).map_err(|source| CliError::CreateDir {
        path: dest.to_path_buf(),
        source,
    })?;

    for entry in walkdir::WalkDir::new(source) {
        let entry = entry.map_err(|err| CliError::ReadDir {
            path: source.to_path_buf(),
            source: std::io::Error::other(err),
        })?;
        let path = entry.path();
        let relative = path
            .strip_prefix(source)
            .map_err(|_| CliError::InvalidProjectPath {
                path: path.to_path_buf(),
            })?;
        let target = dest.join(relative);

        if path.is_dir() {
            std::fs::create_dir_all(&target).map_err(|source| CliError::CreateDir {
                path: target,
                source,
            })?;
            continue;
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|source| CliError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        std::fs::copy(path, &target).map_err(|source| CliError::WriteFile {
            path: target,
            source,
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::paths::factorio_mods_dir;

    #[test]
    fn factorio_mods_dir_defaults_to_home_factorio_mods() {
        let mods_dir = factorio_mods_dir().unwrap();
        assert!(mods_dir.ends_with(".factorio/mods"));
    }
}
