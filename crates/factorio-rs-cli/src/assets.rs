use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;

use crate::{
    config::{AssetEntry, Config},
    error::{CliError, CliResult},
};

/// Default Factorio mod portal thumbnail filename.
pub const DEFAULT_THUMBNAIL: &str = "thumbnail.png";

const RESERVED_ROOT_FILES: &[&str] = &[
    "info.json",
    "control.lua",
    "data.lua",
    "data-updates.lua",
    "data-final-fixes.lua",
    "settings.lua",
    "settings-updates.lua",
    "settings-final-fixes.lua",
    DEFAULT_THUMBNAIL,
];

const RESERVED_ROOT_DIRS: &[&str] = &["lua", "locale"];

/// Copy the mod thumbnail into `output_dir` as `thumbnail.png` when configured
/// or when the default file exists at the project root.
///
/// - If `[mod].thumbnail` is set, the path is required and copied to
///   `output_dir/thumbnail.png`.
/// - Otherwise, `thumbnail.png` at the project root is copied when present.
pub fn copy_thumbnail(
    project_root: &Path,
    output_dir: &Path,
    config: &Config,
) -> CliResult<Option<PathBuf>> {
    let (source, required) = config.r#mod.thumbnail.as_deref().map_or_else(
        || (project_root.join(DEFAULT_THUMBNAIL), false),
        |path| (project_root.join(path), true),
    );

    if !source.is_file() {
        if required {
            return Err(CliError::NotFound { path: source });
        }
        return Ok(None);
    }

    let dest = output_dir.join(DEFAULT_THUMBNAIL);
    std::fs::copy(&source, &dest).map_err(|err| CliError::WriteFile {
        path: dest.clone(),
        source: err,
    })?;
    Ok(Some(dest))
}

/// Copy configured `[mod].assets` into `output_dir`.
///
/// Each entry is required to exist. Directories are copied recursively;
/// destination paths must stay inside `output_dir` and must not collide with
/// generated mod layout (`lua/`, `locale/`, stage entry Lua, `info.json`,
/// `thumbnail.png`).
pub fn copy_assets(
    project_root: &Path,
    output_dir: &Path,
    config: &Config,
) -> CliResult<Vec<PathBuf>> {
    let mut outputs = Vec::new();
    for entry in &config.r#mod.assets {
        outputs.extend(copy_asset_entry(project_root, output_dir, entry)?);
    }
    Ok(outputs)
}

fn copy_asset_entry(
    project_root: &Path,
    output_dir: &Path,
    entry: &AssetEntry,
) -> CliResult<Vec<PathBuf>> {
    let (from, to) = entry.paths();
    let source = project_root.join(from);
    if !source.exists() {
        return Err(CliError::NotFound { path: source });
    }

    let dest_rel = normalize_dest(to)?;
    validate_dest(&dest_rel)?;

    if source.is_file() {
        let dest = output_dir.join(&dest_rel);
        copy_file(&source, &dest)?;
        return Ok(vec![dest]);
    }

    if source.is_dir() {
        return copy_directory(&source, output_dir, &dest_rel);
    }

    Err(CliError::NotFound { path: source })
}

fn copy_directory(
    source_dir: &Path,
    output_dir: &Path,
    dest_rel: &Path,
) -> CliResult<Vec<PathBuf>> {
    validate_dest(dest_rel)?;
    let mut outputs = Vec::new();

    for entry in WalkDir::new(source_dir) {
        let entry = entry.map_err(|err| CliError::ReadDir {
            path: source_dir.to_path_buf(),
            source: err
                .into_io_error()
                .unwrap_or_else(|| std::io::Error::other("failed to walk asset directory")),
        })?;
        let path = entry.path();
        if path == source_dir {
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let rel = path
            .strip_prefix(source_dir)
            .map_err(|_| CliError::InvalidAsset {
                message: format!("asset path `{}` escapes source directory", path.display()),
            })?;
        let file_dest_rel = dest_rel.join(rel);
        validate_dest(&file_dest_rel)?;
        let dest = output_dir.join(&file_dest_rel);
        copy_file(path, &dest)?;
        outputs.push(dest);
    }

    Ok(outputs)
}

fn copy_file(source: &Path, dest: &Path) -> CliResult<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|err| CliError::CreateDir {
            path: parent.to_path_buf(),
            source: err,
        })?;
    }
    std::fs::copy(source, dest).map_err(|err| CliError::WriteFile {
        path: dest.to_path_buf(),
        source: err,
    })?;
    Ok(())
}

/// Normalize a destination relative path: reject absolutes and `..`.
fn normalize_dest(to: &str) -> CliResult<PathBuf> {
    let path = Path::new(to);
    if path.is_absolute() {
        return Err(CliError::InvalidAsset {
            message: format!("destination `{to}` must be relative to the mod output"),
        });
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(CliError::InvalidAsset {
                    message: format!("destination `{to}` must not contain `..`"),
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(CliError::InvalidAsset {
                    message: format!("destination `{to}` must be relative to the mod output"),
                });
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(CliError::InvalidAsset {
            message: "destination path must not be empty".to_string(),
        });
    }

    Ok(normalized)
}

fn validate_dest(dest_rel: &Path) -> CliResult<()> {
    let Some(first) = dest_rel.components().next() else {
        return Err(CliError::InvalidAsset {
            message: "destination path must not be empty".to_string(),
        });
    };
    let Component::Normal(first_name) = first else {
        return Err(CliError::InvalidAsset {
            message: format!(
                "destination `{}` must be relative to the mod output",
                dest_rel.display()
            ),
        });
    };
    let first_name = first_name.to_string_lossy();

    if RESERVED_ROOT_DIRS
        .iter()
        .any(|dir| first_name.eq_ignore_ascii_case(dir))
    {
        return Err(CliError::InvalidAsset {
            message: format!(
                "destination `{}` collides with generated `{first_name}/`",
                dest_rel.display()
            ),
        });
    }

    // Only root-level reserved filenames are blocked (e.g. `thumbnail.png`).
    if dest_rel.components().count() == 1
        && RESERVED_ROOT_FILES
            .iter()
            .any(|name| first_name.eq_ignore_ascii_case(name))
    {
        return Err(CliError::InvalidAsset {
            message: format!(
                "destination `{}` collides with generated `{first_name}`",
                dest_rel.display()
            ),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModConfig;
    use std::collections::BTreeMap;

    fn config_with(thumbnail: Option<&str>, assets: Vec<AssetEntry>) -> Config {
        Config {
            source: "src".to_string(),
            output_dir: "dist".to_string(),
            emit: crate::config::emit::EmitConfig::default(),
            r#mod: ModConfig {
                title: None,
                description: None,
                factorio_version: Some("2.0".to_string()),
                thumbnail: thumbnail.map(str::to_string),
                assets,
                dependencies: Vec::new(),
                emit_api: false,
                api_dir: "api".to_string(),
            },
            profiles: BTreeMap::default(),
            lints: crate::config::lints::LintsConfig::default(),
        }
    }

    fn config_with_thumbnail(thumbnail: Option<&str>) -> Config {
        config_with(thumbnail, Vec::new())
    }

    #[test]
    fn copies_default_thumbnail_when_present() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(root.join("thumbnail.png"), b"png").unwrap();

        let copied = copy_thumbnail(root, &output, &config_with_thumbnail(None))
            .unwrap()
            .unwrap();
        assert_eq!(copied, output.join("thumbnail.png"));
        assert_eq!(std::fs::read(copied).unwrap(), b"png");
    }

    #[test]
    fn skips_missing_default_thumbnail() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::create_dir_all(&output).unwrap();

        let copied = copy_thumbnail(root, &output, &config_with_thumbnail(None)).unwrap();
        assert!(copied.is_none());
    }

    #[test]
    fn errors_when_configured_thumbnail_missing() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::create_dir_all(&output).unwrap();

        let err =
            copy_thumbnail(root, &output, &config_with_thumbnail(Some("missing.png"))).unwrap_err();
        assert!(matches!(err, CliError::NotFound { .. }));
    }

    #[test]
    fn copies_configured_thumbnail_path() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(root.join("assets/thumb.png"), b"png").unwrap();

        let copied = copy_thumbnail(
            root,
            &output,
            &config_with_thumbnail(Some("assets/thumb.png")),
        )
        .unwrap()
        .unwrap();
        assert_eq!(copied, output.join("thumbnail.png"));
        assert_eq!(std::fs::read(copied).unwrap(), b"png");
    }

    #[test]
    fn copies_asset_directory_preserving_path() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::create_dir_all(root.join("graphics/entity")).unwrap();
        std::fs::write(root.join("graphics/entity/foo.png"), b"img").unwrap();
        std::fs::create_dir_all(&output).unwrap();

        let copied = copy_assets(
            root,
            &output,
            &config_with(None, vec![AssetEntry::Path("graphics".into())]),
        )
        .unwrap();

        assert_eq!(copied, vec![output.join("graphics/entity/foo.png")]);
        assert_eq!(
            std::fs::read(output.join("graphics/entity/foo.png")).unwrap(),
            b"img"
        );
    }

    #[test]
    fn remaps_assets_directory() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::create_dir_all(root.join("assets/graphics")).unwrap();
        std::fs::write(root.join("assets/graphics/icon.png"), b"icon").unwrap();
        std::fs::create_dir_all(&output).unwrap();

        let copied = copy_assets(
            root,
            &output,
            &config_with(
                None,
                vec![AssetEntry::Map {
                    from: "assets/graphics".into(),
                    to: "graphics".into(),
                }],
            ),
        )
        .unwrap();

        assert_eq!(copied, vec![output.join("graphics/icon.png")]);
        assert_eq!(
            std::fs::read(output.join("graphics/icon.png")).unwrap(),
            b"icon"
        );
    }

    #[test]
    fn remaps_single_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets/extra.png"), b"extra").unwrap();
        std::fs::create_dir_all(&output).unwrap();

        let copied = copy_assets(
            root,
            &output,
            &config_with(
                None,
                vec![AssetEntry::Map {
                    from: "assets/extra.png".into(),
                    to: "graphics/extra.png".into(),
                }],
            ),
        )
        .unwrap();

        assert_eq!(copied, vec![output.join("graphics/extra.png")]);
        assert_eq!(
            std::fs::read(output.join("graphics/extra.png")).unwrap(),
            b"extra"
        );
    }

    #[test]
    fn errors_when_asset_source_missing() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::create_dir_all(&output).unwrap();

        let err = copy_assets(
            root,
            &output,
            &config_with(None, vec![AssetEntry::Path("graphics".into())]),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::NotFound { .. }));
    }

    #[test]
    fn rejects_parent_dir_escape() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::write(root.join("evil.png"), b"x").unwrap();
        std::fs::create_dir_all(&output).unwrap();

        let err = copy_assets(
            root,
            &output,
            &config_with(
                None,
                vec![AssetEntry::Map {
                    from: "evil.png".into(),
                    to: "../evil.png".into(),
                }],
            ),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::InvalidAsset { .. }));
    }

    #[test]
    fn rejects_reserved_destination() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let output = root.join("dist");
        std::fs::write(root.join("hack.lua"), b"--").unwrap();
        std::fs::create_dir_all(&output).unwrap();

        let err = copy_assets(
            root,
            &output,
            &config_with(
                None,
                vec![AssetEntry::Map {
                    from: "hack.lua".into(),
                    to: "control.lua".into(),
                }],
            ),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::InvalidAsset { .. }));

        let err = copy_assets(
            root,
            &output,
            &config_with(
                None,
                vec![AssetEntry::Map {
                    from: "hack.lua".into(),
                    to: "lua/hack.lua".into(),
                }],
            ),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::InvalidAsset { .. }));
    }

    #[test]
    fn parses_asset_entries_from_toml() {
        let toml = r#"
            [mod]
            assets = [
              "graphics",
              { from = "assets/sounds", to = "sounds" },
            ]
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.r#mod.assets,
            vec![
                AssetEntry::Path("graphics".into()),
                AssetEntry::Map {
                    from: "assets/sounds".into(),
                    to: "sounds".into(),
                },
            ]
        );
    }
}
