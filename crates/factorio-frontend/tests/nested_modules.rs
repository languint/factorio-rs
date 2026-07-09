mod common;

use common::{must_ok, must_ok_parse};
use factorio_codegen::LuaGenerator;
use factorio_frontend::parse_module;
use factorio_ir::{expression::Expression, statement::Statement};

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
pub fn on_init() {
    let mut player = crate::shared::player::MyPlayer::new();

    player.set_health(player.get_health() - 1);
}
";

#[test]
fn nested_player_modules_generate_expected_lua() {
    let player_module = must_ok_parse(parse_module(PLAYER_RS, "shared.player"));
    let health_module = must_ok_parse(parse_module(HEALTH_RS, "shared.player.health"));
    let on_init_module = must_ok_parse(parse_module(ON_INIT_RS, "control.on_init"));

    assert_eq!(
        player_module.submodules,
        vec!["shared.player.health".to_string()]
    );

    let Statement::StructDecl(player_struct) = &player_module.symbols[0].statement else {
        assert_eq!(1, 0, "expected struct");
        return;
    };
    let Some(new_method) = player_struct
        .methods
        .iter()
        .find(|method| method.name == "new")
    else {
        assert_eq!(1, 0, "new method not found");
        return;
    };
    let Statement::Return(Some(Expression::StructLiteral { fields })) =
        &new_method.body.statements[0]
    else {
        assert_eq!(1, 0, "expected struct literal return");
        return;
    };
    assert_eq!(
        fields[0].1,
        Expression::QualifiedPath {
            segments: vec!["MyPlayer".to_string(), "DEFAULT_HEALTH".to_string()],
        }
    );

    let player_lua = must_ok(LuaGenerator::new().generate_module(&player_module));
    assert!(player_lua.contains("require(\"__mod__/lua/shared/player/health\")"));
    assert!(player_lua.contains(
        "setmetatable({ health = sharedPlayer.MyPlayer.DEFAULT_HEALTH }, { __index = sharedPlayer.MyPlayer })"
    ));

    let health_lua = must_ok(LuaGenerator::new().generate_module(&health_module));
    assert!(health_lua.contains("local shared_player = require(\"__mod__/lua/shared/player\")"));
    assert!(health_lua.contains("function MyPlayer:get_health()"));
    assert!(health_lua.contains("function MyPlayer:set_health(health)"));

    assert_eq!(on_init_module.imports.len(), 1);
    assert_eq!(on_init_module.imports[0].module, "shared.player");
    assert_eq!(on_init_module.imports[0].local, "shared_player");

    let on_init_lua = must_ok(LuaGenerator::new().generate_module(&on_init_module));
    assert!(on_init_lua.contains("local shared_player = require(\"__mod__/lua/shared/player\")"));
    assert!(on_init_lua.contains("local player = shared_player.MyPlayer.new()"));
}
