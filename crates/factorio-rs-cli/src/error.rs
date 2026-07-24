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

    #[error("invalid asset path: {message}")]
    InvalidAsset { message: String },

    #[error(
        "library exports missing at `{path}`; run `factorio-rs build` in the library project first \
         (writes `[package.metadata.factorio]` and `src/factorio_exports.rs`)"
    )]
    ExportsManifestMissing { path: PathBuf },

    #[error("invalid project path `{path}`")]
    InvalidProjectPath { path: PathBuf },

    #[error("no Rust source files found in `{path}`")]
    NoSourceFiles { path: PathBuf },

    #[error("failed to parse `{path}`: {source}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("invalid `[lints]` configuration: {message}")]
    InvalidLints { message: String },

    #[error("transpile failed due to previous errors")]
    Reported,

    #[error("typecheck failed (`cargo check`)")]
    TypecheckFailed,

    #[error("macro expansion failed: {message}")]
    MacroExpandFailed { message: String },

    #[error("failed to parse `{path}`: {source}")]
    CargoManifestParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("failed to serialize info.json")]
    InfoJsonSerialize { source: serde_json::Error },

    #[error("failed to resolve Factorio Cargo metadata: {message}")]
    CargoMetadata { message: String },

    #[error("failed to write zip archive `{path}`")]
    ZipWrite {
        path: PathBuf,
        source: zip::result::ZipError,
    },

    #[error("Factorio was not found on this system ({hint})")]
    FactorioNotFound { hint: String },

    #[error(
        "Factorio binary required for `factorio-rs test` / `factorio-rs bench` \
         (Steam protocol launch cannot pass server args). \
         Set FACTORIO_PATH to the Factorio executable."
    )]
    FactorioBinaryRequired,

    #[error("no `#[test]` functions found under `#[cfg(test)]` modules")]
    NoTests,

    #[error("no `#[factorio_rs::bench]` functions found")]
    NoBenches,

    #[error("Factorio test suite timed out after {timeout_secs}s")]
    TestTimeout { timeout_secs: u64 },

    #[error("Factorio test suite failed")]
    TestsFailed,

    #[error("Factorio bench suite timed out after {timeout_secs}s")]
    BenchTimeout { timeout_secs: u64 },

    #[error("Factorio bench suite failed")]
    BenchesFailed,

    #[error("failed to launch Factorio (`{target}`)")]
    LaunchFactorio {
        target: String,
        source: std::io::Error,
    },

    #[error("failed to edit `{path}`: {message}")]
    TomlEdit { path: PathBuf, message: String },

    #[error("{message}")]
    InvalidArgs { message: String },

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
            .ok_or(CliError::InvalidProjectPath { path });
    }

    Err(CliError::NotFound { path })
}
