use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    cargo_manifest::CargoPackage,
    commands::build::{BuildOptions, build},
    config::Config,
    error::{CliError, CliResult},
};

/// Build the mod and write `{name}_{version}.zip` at the project root.
pub fn package(project_root: &Path, options: &BuildOptions) -> CliResult<PathBuf> {
    build(project_root, options)?;
    create_archive(project_root)
}

pub fn create_archive(project_root: &Path) -> CliResult<PathBuf> {
    let package = CargoPackage::load(project_root)?;
    let config = Config::load(project_root)?;
    let output_dir = project_root.join(&config.output_dir);
    let zip_path = project_root.join(format!("{}_{}.zip", package.name, package.version));

    write_zip(
        &output_dir,
        &zip_path,
        &format!("{}_{}", package.name, package.version),
    )?;
    Ok(zip_path)
}

fn write_zip(source_dir: &Path, zip_path: &Path, root_dir: &str) -> CliResult<()> {
    let file = File::create(zip_path).map_err(|source| CliError::WriteFile {
        path: zip_path.to_path_buf(),
        source,
    })?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let root_prefix = format!("{root_dir}/");

    for entry in WalkDir::new(source_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path == source_dir {
            continue;
        }

        let relative = path
            .strip_prefix(source_dir)
            .map_err(|_| CliError::InvalidProjectPath {
                path: path.to_path_buf(),
            })?;
        let name = format!(
            "{root_prefix}{}",
            relative.to_string_lossy().replace('\\', "/")
        );

        if path.is_dir() {
            writer
                .add_directory(format!("{name}/"), options)
                .map_err(|source| CliError::ZipWrite {
                    path: zip_path.to_path_buf(),
                    source,
                })?;
            continue;
        }

        writer
            .start_file(name, options)
            .map_err(|source| CliError::ZipWrite {
                path: zip_path.to_path_buf(),
                source,
            })?;
        let contents = std::fs::read(path).map_err(|source| CliError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        writer
            .write_all(&contents)
            .map_err(|source| CliError::ZipWrite {
                path: zip_path.to_path_buf(),
                source: zip::result::ZipError::Io(source),
            })?;
    }

    writer.finish().map_err(|source| CliError::ZipWrite {
        path: zip_path.to_path_buf(),
        source,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    #[test]
    fn zip_archive_uses_factorio_root_directory_layout() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("dist");
        std::fs::create_dir_all(source_dir.join("lua")).unwrap();
        std::fs::write(source_dir.join("info.json"), r#"{"name":"demo"}"#).unwrap();
        std::fs::write(source_dir.join("control.lua"), "-- demo").unwrap();
        std::fs::write(source_dir.join("lua/demo.lua"), "return {}").unwrap();

        let zip_path = temp_dir.path().join("demo_0.1.0.zip");
        write_zip(&source_dir, &zip_path, "demo_0.1.0").unwrap();

        let file = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut info = archive.by_name("demo_0.1.0/info.json").unwrap();
        let mut contents = String::new();
        info.read_to_string(&mut contents).unwrap();
        assert!(contents.contains("\"name\":\"demo\""));
    }
}
