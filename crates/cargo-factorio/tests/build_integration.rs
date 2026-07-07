use std::path::Path;

const FACTORIO_TOML: &str = "source = \"src\"\noutput_dir = \"lua\"\n";

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
use crate::player::MyPlayer;

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
pub fn on_init() {
    let mut player = crate::player::MyPlayer::new();

    player.set_health(player.get_health() - 1);
}
";

fn write_nested_module_project(project_root: &Path) {
    std::fs::write(project_root.join("Factorio.toml"), FACTORIO_TOML).unwrap();
    std::fs::create_dir_all(project_root.join("src/player")).unwrap();
    std::fs::write(project_root.join("src/on_init.rs"), ON_INIT_RS).unwrap();
    std::fs::write(project_root.join("src/player.rs"), PLAYER_RS).unwrap();
    std::fs::write(project_root.join("src/player/health.rs"), HEALTH_RS).unwrap();
}

#[test]
fn build_generates_nested_module_lua() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path();
    write_nested_module_project(project_root);

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .arg("build")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());

    let on_init_lua = std::fs::read_to_string(project_root.join("lua/on_init.lua")).unwrap();
    assert!(
        on_init_lua.contains("local player = require(\"player\")"),
        "generated lua:\n{on_init_lua}"
    );

    let player_lua = std::fs::read_to_string(project_root.join("lua/player.lua")).unwrap();
    assert!(player_lua.contains("require(\"player.health\")"));

    let health_lua =
        std::fs::read_to_string(project_root.join("lua/player/health.lua")).unwrap();
    assert!(health_lua.contains("function MyPlayer:get_health()"));
}

#[test]
fn build_removes_stale_lua_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path();
    write_nested_module_project(project_root);

    let stale_path = project_root.join("lua/stale.lua");
    std::fs::create_dir_all(project_root.join("lua")).unwrap();
    std::fs::write(&stale_path, "stale").unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_cargo-factorio"))
        .arg("build")
        .current_dir(project_root)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!stale_path.exists());
}
