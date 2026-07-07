use factorio_codegen::LuaGenerator;
use factorio_frontend::parse_module;

#[test]
fn parses_file_based_submodule_declaration() {
    let source = r"
mod extra_info;

pub struct MyPlayer {
    health: u64,
}
";

    let module = parse_module(source, "player").unwrap();

    assert_eq!(module.submodules, vec!["player.extra_info".to_string()]);
}

#[test]
fn generates_require_for_declared_submodules() {
    let source = r"
mod extra_info;

pub fn on_init() {}
";

    let module = parse_module(source, "player").unwrap();
    let lua = LuaGenerator::new().generate_module(&module).unwrap();

    assert!(lua.contains("require(\"player.extra_info\")"));
}

#[test]
fn parses_submodule_source_with_parent_import() {
    let source = r"
use crate::player::MyPlayer;

impl MyPlayer {
    pub fn get_health(&self) -> u64 {
        self.health
    }
}
";

    let module = parse_module(source, "player.extra_info").unwrap();
    let lua = LuaGenerator::new().generate_module(&module).unwrap();

    assert_eq!(module.imports.len(), 1);
    assert_eq!(module.imports[0].module, "player");
    assert!(lua.contains("local player = require(\"player\")"));
    assert!(lua.contains("local MyPlayer = player.MyPlayer"));
    assert!(!lua.contains("local MyPlayer = {}"));
    assert!(lua.contains("function MyPlayer:get_health()"));
}
