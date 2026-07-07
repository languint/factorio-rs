use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("failed to read `{path}`")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write `{path}`")]
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to create `{path}`")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read directory `{path}`")]
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to remove `{path}`")]
    RemoveDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("`{path}` already exists")]
    AlreadyExists { path: PathBuf },

    #[error("could not find `{path}`")]
    NotFound { path: PathBuf },

    #[error("invalid project path `{path}`")]
    InvalidProjectPath { path: PathBuf },

    #[error("no Rust source files found in `{path}`")]
    NoSourceFiles { path: PathBuf },

    #[error("failed to parse `{path}`")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error(transparent)]
    Frontend(#[from] factorio_frontend::FrontendError),

    #[error(transparent)]
    Codegen(#[from] factorio_codegen::LuaGeneratorError),
}

pub type CliResult<T> = Result<T, CliError>;

pub fn project_root(manifest_path: Option<&Path>) -> CliResult<PathBuf> {
    let path = match manifest_path {
        None => std::env::current_dir().map_err(|source| CliError::ReadDir {
            path: PathBuf::from("."),
            source,
        })?,
        Some(path) => path.to_path_buf(),
    };

    if path.is_dir() {
        return Ok(path);
    }

    if path.is_file() {
        return path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| CliError::InvalidProjectPath { path });
    }

    Err(CliError::NotFound { path })
}
