mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::statement::Statement;

const PLAYER_SOURCE: &str = r"
pub struct MyPlayer {
    health: i64,
}

impl MyPlayer {
    pub fn get_health(&self) -> i64 {
        self.health
    }

    pub fn set_health(&mut self, health: i64) {
        self.health = health;
    }
}
";

#[test]
fn parses_struct_with_methods() {
    let module = must_ok_parse(parse_module(PLAYER_SOURCE, "shared.player"));

    let Statement::StructDecl(struct_decl) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected struct declaration");
        return;
    };

    assert_eq!(struct_decl.name, "MyPlayer");
    assert_eq!(struct_decl.fields.len(), 1);
    assert_eq!(struct_decl.fields[0].name, "health");
    assert_eq!(struct_decl.fields[0].source_type.as_deref(), Some("i64"));
    assert_eq!(
        struct_decl.debug.as_ref().map(|debug| debug.header_comment.as_str()),
        Some("pub struct MyPlayer { health: i64 }")
    );

    assert_eq!(struct_decl.methods.len(), 2);
    assert_eq!(struct_decl.methods[0].name, "get_health");
    assert_eq!(
        struct_decl.methods[0].params[0].source_type.as_deref(),
        Some("&self")
    );
    assert_eq!(
        struct_decl.methods[0]
            .debug
            .as_ref()
            .and_then(|debug| debug.return_type.as_deref()),
        Some("i64")
    );

    assert_eq!(struct_decl.methods[1].name, "set_health");
    assert_eq!(
        struct_decl.methods[1].params[1].source_type.as_deref(),
        Some("i64")
    );
}
