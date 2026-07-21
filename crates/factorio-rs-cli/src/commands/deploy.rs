//! Deploy a built mod directory into a Factorio mods folder (copy or symlink).

use std::path::{Path, PathBuf};

use crate::error::{CliError, CliResult};

/// How to place the mod at the destination path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployMode {
    /// Recursive file copy (always available).
    Copy,
    /// Symlink the destination to the source directory (Unix); falls back to copy elsewhere.
    Symlink,
}

/// Remove `dest` if it exists (file, directory, or symlink), then deploy `source` there.
///
/// Returns the mode actually used (`Symlink` may fall back to `Copy`).
///
/// When `dest` is already a symlink to `source`, the existing link is left alone so
/// Factorio does not briefly lose the mod during hot-reload syncs.
pub fn deploy_mod(source: &Path, dest: &Path, mode: DeployMode) -> CliResult<DeployMode> {
    match mode {
        DeployMode::Copy => {
            remove_dest(dest)?;
            copy_dir_recursive(source, dest)?;
            Ok(DeployMode::Copy)
        }
        DeployMode::Symlink => {
            if symlink_already_points_at(source, dest)? {
                return Ok(DeployMode::Symlink);
            }
            remove_dest(dest)?;
            if try_symlink(source, dest)? {
                Ok(DeployMode::Symlink)
            } else {
                copy_dir_recursive(source, dest)?;
                Ok(DeployMode::Copy)
            }
        }
    }
}

fn symlink_already_points_at(source: &Path, dest: &Path) -> CliResult<bool> {
    #[cfg(unix)]
    {
        let meta = match std::fs::symlink_metadata(dest) {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(source) => {
                return Err(CliError::ReadFile {
                    path: dest.to_path_buf(),
                    source,
                });
            }
        };
        if !meta.file_type().is_symlink() {
            return Ok(false);
        }
        let abs_source = std::fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
        let abs_dest = std::fs::canonicalize(dest).unwrap_or_else(|_| dest.to_path_buf());
        Ok(abs_source == abs_dest)
    }
    #[cfg(not(unix))]
    {
        let _ = (source, dest);
        Ok(false)
    }
}

fn try_symlink(source: &Path, dest: &Path) -> CliResult<bool> {
    #[cfg(unix)]
    {
        let abs = std::fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|err| CliError::CreateDir {
                path: parent.to_path_buf(),
                source: err,
            })?;
        }
        match std::os::unix::fs::symlink(&abs, dest) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (source, dest);
        Ok(false)
    }
}

fn remove_dest(dest: &Path) -> CliResult<()> {
    if !dest.exists() && !dest.is_symlink() {
        return Ok(());
    }

    let meta = std::fs::symlink_metadata(dest).map_err(|source| CliError::ReadFile {
        path: dest.to_path_buf(),
        source,
    })?;

    if meta.file_type().is_symlink() || meta.is_file() {
        std::fs::remove_file(dest).map_err(|source| CliError::RemoveDir {
            path: dest.to_path_buf(),
            source,
        })?;
    } else {
        std::fs::remove_dir_all(dest).map_err(|source| CliError::RemoveDir {
            path: dest.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

/// Recursively copy `source` into `dest` (creates `dest`).
pub fn copy_dir_recursive(source: &Path, dest: &Path) -> CliResult<()> {
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

/// Mods folder destination: `{mods_dir}/{name}_{version}`.
#[must_use]
pub fn mod_dest(mods_dir: &Path, name: &str, version: &str) -> PathBuf {
    mods_dir.join(format!("{name}_{version}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn copy_deploys_files() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("lua")).unwrap();
        std::fs::write(src.join("info.json"), "{}").unwrap();
        std::fs::write(src.join("lua").join("a.lua"), "return 1").unwrap();

        let mode = deploy_mod(&src, &dst, DeployMode::Copy).unwrap();
        assert_eq!(mode, DeployMode::Copy);
        assert!(dst.join("info.json").is_file());
        assert!(dst.join("lua/a.lua").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_deploys_link() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("info.json"), "{}").unwrap();

        let mode = deploy_mod(&src, &dst, DeployMode::Symlink).unwrap();
        assert_eq!(mode, DeployMode::Symlink);
        assert!(dst.is_symlink());
        assert!(dst.join("info.json").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_skips_recreate_when_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("info.json"), "{}").unwrap();

        deploy_mod(&src, &dst, DeployMode::Symlink).unwrap();
        let first_ino = std::fs::symlink_metadata(&dst).unwrap();
        deploy_mod(&src, &dst, DeployMode::Symlink).unwrap();
        let second_ino = std::fs::symlink_metadata(&dst).unwrap();
        assert_eq!(
            first_ino.modified().unwrap(),
            second_ino.modified().unwrap()
        );
        assert!(dst.is_symlink());
    }
}
