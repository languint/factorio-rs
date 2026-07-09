#![allow(clippy::unwrap_used)]

use std::path::Path;

const FACTORIO_TOML: &str = r#"
prune_dead_code = true

[mod]
title = "Test Mod"
"#;

const CARGO_TOML: &str = r#"
[package]
name = "test-mod"
version = "0.1.0"
authors = ["test@example.com"]
"#;

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

const ON_INIT_RS: &str = r"
#[factorio_rs::event(OnInit)]
pub fn on_init() {
    let mut player = crate::shared::player::MyPlayer::new();

    player.set_health(player.get_health() - 1);
}
";

fn write_nested_module_project(project_root: &Path) {
    std::fs::write(project_root.join("Factorio.toml"), FACTORIO_TOML).unwrap();
    std::fs::write(project_root.join("Cargo.toml"), CARGO_TOML).unwrap();
    std::fs::create_dir_all(project_root.join("src/control")).unwrap();
    std::fs::create_dir_all(project_root.join("src/shared/player")).unwrap();
    std::fs::write(project_root.join("src/control/on_init.rs"), ON_INIT_RS).unwrap();
    std::fs::write(project_root.join("src/shared/player.rs"), PLAYER_RS).unwrap();
    std::fs::write(
        project_root.join("src/shared/player/health.rs"),
        HEALTH_RS,
    )
    .unwrap();
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

    let on_init_lua =
        std::fs::read_to_string(project_root.join("dist/lua/control/on_init.lua")).unwrap();
    assert!(
        on_init_lua.contains("local shared_player = require(\"__test-mod__/lua/shared/player\")"),
        "generated lua:\n{on_init_lua}"
    );

    let player_lua =
        std::fs::read_to_string(project_root.join("dist/lua/shared/player.lua")).unwrap();
    assert!(
        player_lua.contains("require(\"__test-mod__/lua/shared/player/health\")")
    );

    let health_lua =
        std::fs::read_to_string(project_root.join("dist/lua/shared/player/health.lua")).unwrap();
    assert!(health_lua.contains("function MyPlayer:get_health()"));

    assert!(project_root.join("dist/control.lua").is_file());
    assert!(project_root.join("dist/info.json").is_file());
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
