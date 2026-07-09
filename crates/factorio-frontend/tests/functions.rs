mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::statement::Statement;

#[test]
fn parses_method_with_self() {
    let source = r"
pub fn reset(&mut self, player: ()) {
    return;
}
";

    let module = must_ok_parse(parse_module(source, "control.player_util"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function declaration");
        return;
    };

    assert_eq!(function.name, "reset");
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].source_type.as_deref(), Some("&mut self"));
    assert_eq!(function.params[1].source_type.as_deref(), Some("()"));
    assert_eq!(
        function.body.statements,
        vec![Statement::Return(None)]
    );
}

#[test]
fn parses_implicit_return() {
    let source = r"
fn helper() -> i64 {
    1
}
";

    let module = must_ok_parse(parse_module(source, "control.example"));
    let Statement::FunctionDecl(function) = &module.body.statements[0] else {
        assert_eq!(1, 0, "expected helper function");
        return;
    };

    assert_eq!(function.name, "helper");
    assert_eq!(
        function.debug.as_ref().and_then(|debug| debug.return_type.as_deref()),
        Some("i64")
    );
}
