#![allow(
    clippy::expect_used,
    clippy::literal_string_with_formatting_args,
    clippy::needless_raw_string_hashes,
    clippy::panic,
    clippy::unwrap_used
)]

use std::path::Path;

const FACTORIO_TOML: &str = r#"
[mod]
title = "Test Mod"

[profiles.release]
prune_dead_code = true
"#;

fn cargo_toml() -> String {
    let factorio_rs = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../factorio-rs")
        .canonicalize()
        .expect("factorio-rs crate path");
    format!(
        r#"
[package]
name = "test-mod"
version = "0.1.0"
edition = "2024"
authors = ["test@example.com"]

[lib]
path = "src/lib.rs"

[dependencies]
factorio-rs = {{ path = "{}" }}
"#,
        factorio_rs.display()
    )
}

const LIB_RS: &str = r"
mod control;
mod shared;
";

const PLAYER_RS: &str = r"
mod health;

pub struct MyPlayer {
    health: u64,
}

impl MyPlayer {
    pub fn new() -> Self {
        Self {
            health: Self::DEFAULT_HEALTH,
        }
    }
}
";

const HEALTH_RS: &str = r"
use crate::shared::player::MyPlayer;

impl MyPlayer {
    pub const DEFAULT_HEALTH: u64 = 100;

    pub fn get_health(&self) -> u64 {
        self.health
    }

    pub fn set_health(&mut self, health: u64) {
        self.health = health;
    }
}
";

const ON_SINGLEPLAYER_INIT_RS: &str = r"
#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let mut player = crate::shared::player::MyPlayer::new();

    player.set_health(player.get_health() - 1);
}
";

fn write_nested_module_project(project_root: &Path) {
    std::fs::write(project_root.join("Factorio.toml"), FACTORIO_TOML).unwrap();
    std::fs::write(project_root.join("Cargo.toml"), cargo_toml()).unwrap();
    std::fs::create_dir_all(project_root.join("src/control")).unwrap();
    std::fs::create_dir_all(project_root.join("src/shared/player")).unwrap();
    std::fs::write(project_root.join("src/lib.rs"), LIB_RS).unwrap();
    std::fs::write(
        project_root.join("src/control/mod.rs"),
        "mod on_singleplayer_init;\n",
    )
    .unwrap();
    std::fs::write(project_root.join("src/shared/mod.rs"), "pub mod player;\n").unwrap();
    std::fs::write(
        project_root.join("src/control/on_singleplayer_init.rs"),
        ON_SINGLEPLAYER_INIT_RS,
    )
    .unwrap();
    std::fs::write(project_root.join("src/shared/player.rs"), PLAYER_RS).unwrap();
    std::fs::write(project_root.join("src/shared/player/health.rs"), HEALTH_RS).unwrap();
}

#[test]
fn build_generates_nested_module_lua() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path();
    write_nested_module_project(project_root);

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .arg("build")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let on_singleplayer_init_lua =
        std::fs::read_to_string(project_root.join("dist/lua/control/on_singleplayer_init.lua"))
            .unwrap();
    assert!(
        on_singleplayer_init_lua
            .contains("local shared_player = require(\"__test-mod__/lua/shared/player\")"),
        "generated lua:\n{on_singleplayer_init_lua}"
    );

    let player_lua =
        std::fs::read_to_string(project_root.join("dist/lua/shared/player.lua")).unwrap();
    assert!(player_lua.contains("require(\"__test-mod__/lua/shared/player/health\")"));

    let health_lua =
        std::fs::read_to_string(project_root.join("dist/lua/shared/player/health.lua")).unwrap();
    assert!(health_lua.contains("function MyPlayer:get_health()"));

    assert!(project_root.join("dist/control.lua").is_file());
    assert!(project_root.join("dist/info.json").is_file());
}

#[test]
fn build_expands_macro_rules_invocations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path();
    std::fs::write(project_root.join("Factorio.toml"), FACTORIO_TOML).unwrap();
    std::fs::write(project_root.join("Cargo.toml"), cargo_toml()).unwrap();
    std::fs::create_dir_all(project_root.join("src")).unwrap();
    std::fs::write(
        project_root.join("src/lib.rs"),
        r#"
#[factorio_rs::control]
mod control {
    macro_rules! shout {
        ($msg:expr) => {
            println!($msg)
        };
    }

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        shout!("macros work");
    }
}
"#,
    )
    .unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .arg("build")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let lua = std::fs::read_to_string(project_root.join("dist/lua/control.lua")).unwrap();
    assert!(
        lua.contains("game.print") && lua.contains("macros work"),
        "expected expanded macro_rules output in lua:\n{lua}"
    );
}

#[test]
fn build_discovers_control_mod_bang() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path();
    std::fs::write(project_root.join("Factorio.toml"), FACTORIO_TOML).unwrap();
    std::fs::write(project_root.join("Cargo.toml"), cargo_toml()).unwrap();
    std::fs::create_dir_all(project_root.join("src")).unwrap();
    std::fs::write(
        project_root.join("src/lib.rs"),
        r#"
factorio_rs::control_mod! {
    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        println!("Initialized");
    }
}
"#,
    )
    .unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .arg("build")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let lua = std::fs::read_to_string(project_root.join("dist/lua/control.lua")).unwrap();
    assert!(
        lua.contains("on_singleplayer_init"),
        "expected control_mod! event in lua:\n{lua}"
    );
    assert!(project_root.join("dist/control.lua").is_file());
}

#[test]
fn build_removes_stale_lua_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path();
    write_nested_module_project(project_root);

    let stale_path = project_root.join("dist/stale.lua");
    std::fs::create_dir_all(project_root.join("dist")).unwrap();
    std::fs::write(&stale_path, "stale").unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_factorio-rs"))
        .arg("build")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!stale_path.exists());
}
