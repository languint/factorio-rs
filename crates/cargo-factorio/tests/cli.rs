#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn write_cargo_patch(project_root: &std::path::Path) {
    let factorio_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/factorio");
    let cargo_dir = project_root.join(".cargo");
    std::fs::create_dir_all(&cargo_dir).unwrap();
    std::fs::write(
        cargo_dir.join("config.toml"),
        format!(
            "[patch.crates-io]\nfactorio = {{ path = \"{}\" }}\n",
            factorio_path.display()
        ),
    )
    .unwrap();
}

#[test]
fn init_creates_cargo_project_files() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();

    assert!(status.success());
    assert!(project_root.join("Cargo.toml").is_file());
    assert!(project_root.join("Factorio.toml").is_file());
    assert!(project_root.join("src/lib.rs").is_file());
    assert!(project_root.join(".gitignore").is_file());
    let lib_rs = std::fs::read_to_string(project_root.join("src/lib.rs")).unwrap();
    assert!(lib_rs.contains("factorio::control_mod!"));
}

#[test]
fn build_generates_lua_from_sources() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_cargo_patch(project_root);

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .arg("build")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let lua_output = project_root.join("dist/lua/control.lua");
    assert!(lua_output.is_file());
    assert!(project_root.join("dist/control.lua").is_file());
    assert!(project_root.join("dist/info.json").is_file());

    let lua = std::fs::read_to_string(lua_output).unwrap();
    assert!(lua.contains("function control.on_init"));
}

#[test]
fn initialized_project_passes_cargo_check() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_cargo_patch(project_root);

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new("cargo")
        .arg("check")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn init_fails_when_project_already_exists() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    let binary = env!("CARGO_BIN_EXE_cargo-factorio");

    let status = Command::new(binary)
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(binary)
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn build_with_package_flag_creates_factorio_zip() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_cargo_patch(project_root);

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .args(["build", "--package"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(project_root.join("test-mod_0.1.0.zip").is_file());
}

#[test]
fn package_creates_factorio_zip() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_cargo_patch(project_root);

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .arg("package")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(project_root.join("test-mod_0.1.0.zip").is_file());
}
