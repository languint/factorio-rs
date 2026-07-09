mod common;

use common::{must_ok, must_ok_parse};
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

    let module = must_ok_parse(parse_module(source, "shared.player"));

    assert_eq!(
        module.submodules,
        vec!["shared.player.extra_info".to_string()]
    );
}

#[test]
fn generates_require_for_declared_submodules() {
    let source = r"
mod extra_info;

pub fn on_init() {}
";

    let module = must_ok_parse(parse_module(source, "shared.player"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains("require(\"__mod__/lua/shared/player/extra_info\")"));
    assert!(lua.contains("package.loaded[\"__mod__/lua/shared/player\"] = sharedPlayer"));
}

#[test]
fn parent_registers_module_before_loading_submodules() {
    let source = r"
mod health;

pub struct MyPlayer {
    health: u64,
}
";

    let module = must_ok_parse(parse_module(source, "shared.player"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    let Some(player_table) = lua.find("local sharedPlayer = {}") else {
        assert_eq!(1, 0, "module table not found");
        return;
    };
    let Some(package_loaded) = lua.find("package.loaded[\"__mod__/lua/shared/player\"] = sharedPlayer") else {
        assert_eq!(1, 0, "early package.loaded registration not found");
        return;
    };
    let Some(submodule_require) = lua.find("require(\"__mod__/lua/shared/player/health\")") else {
        assert_eq!(1, 0, "submodule require not found");
        return;
    };

    assert!(player_table < package_loaded);
    assert!(package_loaded < submodule_require);
}

#[test]
fn submodule_extends_imported_type_without_new_table() {
    let source = r"
use crate::shared::player::MyPlayer;

impl MyPlayer {
    pub const DEFAULT_HEALTH: u64 = 100;

    pub fn get_health(&self) -> u64 {
        self.health
    }
}
";

    let module = must_ok_parse(parse_module(source, "shared.player.health"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains("local MyPlayer = shared_player.MyPlayer"));
    assert!(lua.contains("MyPlayer.DEFAULT_HEALTH = 100"));
    assert!(!lua.contains("local MyPlayer = {}"));
}

#[test]
fn parses_submodule_source_with_parent_import() {
    let source = r"
use crate::shared::player::MyPlayer;

impl MyPlayer {
    pub fn get_health(&self) -> u64 {
        self.health
    }
}
";

    let module = must_ok_parse(parse_module(source, "shared.player.extra_info"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert_eq!(module.imports.len(), 1);
    assert_eq!(module.imports[0].module, "shared.player");
    assert_eq!(module.imports[0].local, "shared_player");
    assert!(lua.contains("local shared_player = require(\"__mod__/lua/shared/player\")"));
    assert!(lua.contains("local MyPlayer = shared_player.MyPlayer"));
    assert!(!lua.contains("local MyPlayer = {}"));
    assert!(lua.contains("function MyPlayer:get_health()"));
}
