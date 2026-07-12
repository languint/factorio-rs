use std::path::{Path, PathBuf};

use crate::{
    config::Config,
    error::{CliError, CliResult},
};

/// Default Factorio mod portal thumbnail filename.
pub const DEFAULT_THUMBNAIL: &str = "thumbnail.png";

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModConfig;
    use std::collections::BTreeMap;

    fn config_with_thumbnail(thumbnail: Option<&str>) -> Config {
        Config {
            source: "src".to_string(),
            output_dir: "dist".to_string(),
            emit: crate::config::emit::EmitConfig::default(),
            r#mod: ModConfig {
                title: None,
                description: None,
                factorio_version: Some("2.0".to_string()),
                thumbnail: thumbnail.map(str::to_string),
            },
            profiles: BTreeMap::default(),
            lints: crate::config::lints::LintsConfig::default(),
        }
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

        let err = copy_thumbnail(root, &output, &config_with_thumbnail(Some("missing.png")))
            .unwrap_err();
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

        let copied =
            copy_thumbnail(root, &output, &config_with_thumbnail(Some("assets/thumb.png")))
                .unwrap()
                .unwrap();
        assert_eq!(copied, output.join("thumbnail.png"));
        assert_eq!(std::fs::read(copied).unwrap(), b"png");
    }
}
