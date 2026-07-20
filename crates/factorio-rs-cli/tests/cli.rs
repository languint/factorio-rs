#![allow(
    clippy::expect_used,
    clippy::literal_string_with_formatting_args,
    clippy::needless_raw_string_hashes,
    clippy::panic,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn write_cargo_patch(project_root: &std::path::Path) {
    let factorio_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/factorio-rs");
    let cargo_dir = project_root.join(".cargo");
    std::fs::create_dir_all(&cargo_dir).unwrap();
    std::fs::write(
        cargo_dir.join("config.toml"),
        format!(
            "[patch.crates-io]\nfactorio-rs = {{ path = \"{}\" }}\n",
            factorio_path.display()
        ),
    )
    .unwrap();
}

#[test]
fn init_creates_cargo_project_files() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
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
    assert!(lib_rs.contains("factorio_rs::control_mod!"));
}

#[test]
fn build_generates_lua_from_sources() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_cargo_patch(project_root);

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
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
    assert!(lua.contains("on_singleplayer_init"));
}

#[test]
fn initialized_project_passes_cargo_check() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_cargo_patch(project_root);

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
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

    let binary = env!("CARGO_BIN_EXE_factorio-rs");

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

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .args(["build", "--package"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(project_root.join("test-mod_0.2.0.zip").is_file());
}

#[test]
fn package_creates_factorio_zip() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_cargo_patch(project_root);

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .arg("package")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(project_root.join("test-mod_0.2.0.zip").is_file());
}

#[test]
fn test_command_runs_with_fake_factorio() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_cargo_patch(project_root);

    let status = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .args(["init", "--name", "test-mod"])
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    std::fs::write(
        project_root.join("src/lib.rs"),
        r#"factorio_rs::control_mod! {
    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        println!("hi");
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn smoke() {
            assert_eq!(1 + 1, 2);
        }
    }
}
"#,
    )
    .unwrap();

    let fake_bin = temp_dir.path().join("fake-factorio");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(
            &fake_bin,
            r#"#!/bin/sh
echo "FACTORIO_RS_TEST start tests::smoke"
echo "FACTORIO_RS_TEST ok tests::smoke"
echo "FACTORIO_RS_TEST suite_end 1 0"
# Stay alive until the runner kills us after suite_end.
sleep 30
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&fake_bin).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_bin, perms).unwrap();
    }
    #[cfg(not(unix))]
    {
        let _ = fake_bin;
        return;
    }

    let output = Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .arg("test")
        .current_dir(project_root)
        .env("FACTORIO_PATH", &fake_bin)
        .env("FACTORIO_RS_NO_STEAM_RUN", "1")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "factorio-rs test failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("test tests::smoke ... ok"),
        "missing pass line in:\n{stdout}"
    );
    assert!(
        project_root
            .join("dist/lua/factorio_rs_tests.lua")
            .is_file(),
        "expected generated test suite lua"
    );
}
