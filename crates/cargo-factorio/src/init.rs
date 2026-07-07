use std::path::Path;

use crate::{
    config::Config,
    error::{CliError, CliResult},
};

const FACTORIO_SDK_VERSION: &str = "0.1.0";

const LIB_RS: &str = r"//! Factorio mod source crate.

mod on_init;
";

const ON_INIT_RS: &str = r"#![allow(dead_code)]

use factorio::event::OnInit;

fn helper() -> i64 {
    1
}

pub fn on_init(_event: OnInit) {
    let _count: i32 = 0;
}
";

const FACTORIO_CONFIG: &str = r#"# cargo-factorio project configuration
source = "src"
output_dir = "lua"
"#;

const GITIGNORE: &str = r"/target
/lua
";

/// Initialize a new cargo-factorio project in `project_root`.
pub fn init(project_root: &Path, package_name: Option<&str>) -> CliResult<()> {
    let cargo_manifest = project_root.join("Cargo.toml");
    if cargo_manifest.exists() {
        return Err(CliError::AlreadyExists {
            path: cargo_manifest,
        });
    }

    let config_path = Config::config_path(project_root);
    if config_path.exists() {
        return Err(CliError::AlreadyExists { path: config_path });
    }

    let package_name = package_name
        .map(str::to_string)
        .unwrap_or_else(|| default_package_name(project_root));

    let source_dir = project_root.join("src");
    let lib_rs = source_dir.join("lib.rs");
    let on_init_rs = source_dir.join("on_init.rs");

    std::fs::create_dir_all(&source_dir).map_err(|source| CliError::CreateDir {
        path: source_dir.clone(),
        source,
    })?;

    write_file(&cargo_manifest, &cargo_toml_template(&package_name))?;
    write_file(&config_path, FACTORIO_CONFIG)?;
    write_file(&lib_rs, LIB_RS)?;
    write_file(&on_init_rs, ON_INIT_RS)?;
    write_file(&project_root.join(".gitignore"), GITIGNORE)?;

    Ok(())
}

fn cargo_toml_template(package_name: &str) -> String {
    format!(
        r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
factorio = "{FACTORIO_SDK_VERSION}"
"#
    )
}

fn default_package_name(project_root: &Path) -> String {
    let fallback = "factorio-mod".to_string();

    let Some(raw_name) = project_root.file_name().and_then(|name| name.to_str()) else {
        return fallback;
    };

    let mut sanitized = String::new();
    for character in raw_name.chars() {
        if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
            sanitized.push(character);
        } else if character == '.' && sanitized.is_empty() {
            continue;
        } else if sanitized.is_empty() || sanitized.ends_with('-') {
            continue;
        } else {
            sanitized.push('-');
        }
    }

    while sanitized.ends_with('-') {
        sanitized.pop();
    }

    if sanitized.is_empty() {
        return fallback;
    }

    if sanitized
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        return format!("mod-{sanitized}");
    }

    sanitized
}

fn write_file(path: &Path, contents: &str) -> CliResult<()> {
    std::fs::write(path, contents).map_err(|source| CliError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{cargo_toml_template, default_package_name};

    #[test]
    fn default_package_name_sanitizes_temp_directories() {
        let name = default_package_name(Path::new("/tmp/.tmpABC123"));
        assert_eq!(name, "tmpABC123");
    }

    #[test]
    fn cargo_toml_includes_factorio_dependency() {
        let manifest = cargo_toml_template("my-mod");
        assert!(manifest.contains("name = \"my-mod\""));
        assert!(manifest.contains("factorio = \"0.1.0\""));
    }
}
