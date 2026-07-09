mod common;

use common::{must_ok, must_ok_parse};
use factorio_codegen::LuaGenerator;
use factorio_frontend::parse_module;
use factorio_ir::statement::Statement;

#[test]
fn parses_and_emits_function_doc_comments() {
    let source = r"
/// Called when the game starts.
pub fn on_init() {}
";

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function");
        return;
    };

    assert_eq!(function.doc.as_deref(), Some("Called when the game starts."));

    let lua = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(lua.contains("-- Called when the game starts."));
    assert!(lua.contains("function controlOnInit.on_init()"));
}

#[test]
fn parses_and_emits_struct_and_method_doc_comments() {
    let source = r"
/// The player type.
pub struct MyPlayer {
    health: u64,
}

impl MyPlayer {
    /// Returns the current health.
    pub fn get_health(&self) -> u64 {
        self.health
    }
}
";

    let module = must_ok_parse(parse_module(source, "shared.player"));
    let Statement::StructDecl(struct_decl) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected struct");
        return;
    };

    assert_eq!(struct_decl.doc.as_deref(), Some("The player type."));
    assert_eq!(
        struct_decl.methods[0].doc.as_deref(),
        Some("Returns the current health.")
    );

    let lua = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(lua.contains("-- The player type."));
    assert!(lua.contains("-- Returns the current health."));
    assert!(lua.contains("function sharedPlayer.MyPlayer:get_health()"));
}

#[test]
fn emits_multiline_doc_comments() {
    let source = r"
/// Line one.
///
/// Line two.
pub fn on_init() {}
";

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains("-- Line one."));
    assert!(lua.contains("--"));
    assert!(lua.contains("-- Line two."));
}
