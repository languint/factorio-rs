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

#[test]
fn nested_player_modules_generate_expected_lua() {
    let player_module = parse_module(PLAYER_RS, "player").unwrap();
    let health_module = parse_module(HEALTH_RS, "player.health").unwrap();
    let on_init_module = parse_module(ON_INIT_RS, "on_init").unwrap();

    assert_eq!(player_module.submodules, vec!["player.health".to_string()]);

    let Statement::StructDecl(player_struct) = &player_module.symbols[0].statement else {
        panic!("expected struct");
    };
    let new_method = player_struct
        .methods
        .iter()
        .find(|method| method.name == "new")
        .expect("new method");
    let Statement::Return(Some(Expression::StructLiteral { fields })) =
        &new_method.body.statements[0]
    else {
        panic!("expected struct literal return");
    };
    assert_eq!(
        fields[0].1,
        Expression::QualifiedPath {
            segments: vec!["MyPlayer".to_string(), "DEFAULT_HEALTH".to_string()],
        }
    );

    let player_lua = LuaGenerator::new()
        .generate_module(&player_module)
        .expect("generate player lua");
    assert!(player_lua.contains("require(\"player.health\")"));

    let health_lua = LuaGenerator::new()
        .generate_module(&health_module)
        .expect("generate health lua");
    assert!(health_lua.contains("local player = require(\"player\")"));
    assert!(health_lua.contains("function MyPlayer:get_health()"));
    assert!(health_lua.contains("function MyPlayer:set_health(health)"));

    assert_eq!(on_init_module.imports.len(), 1);
    assert_eq!(on_init_module.imports[0].module, "player");

    let on_init_lua = LuaGenerator::new()
        .generate_module(&on_init_module)
        .expect("generate on_init lua");
    assert!(on_init_lua.contains("local player = require(\"player\")"));
    assert!(on_init_lua.contains("local player = player.MyPlayer.new()"));
}
