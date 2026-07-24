#![allow(
    clippy::expect_used,
    clippy::needless_raw_string_hashes,
    clippy::panic,
    clippy::unwrap_used
)]
mod common;

use common::must_ok_parse;
use factorio_codegen::LuaGenerator;
use factorio_frontend::{FrontendError, ParseOptions, parse_module, parse_module_with_options};
use factorio_ir::{lint::LintConfig, statement::Statement};

#[test]
fn lua_macro_in_unsafe_fn_lowers_to_raw_lua() {
    let source = r#"
pub unsafe fn patch() {
    lua! {
        local x = 1
        print(x)
    }
}
"#;

    let module = must_ok_parse(parse_module(source, "control"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
    };

    assert_eq!(function.name, "patch");
    assert_eq!(function.body.statements.len(), 1);
    let Statement::RawLua { code } = &function.body.statements[0] else {
        panic!(
            "expected RawLua statement, got {:?}",
            function.body.statements[0]
        );
    };
    assert!(
        code.contains("local x = 1"),
        "code should contain first line, got: {code:?}"
    );
    assert!(
        code.contains("print(x)"),
        "code should contain second line, got: {code:?}"
    );
}

#[test]
fn lua_macro_in_unsafe_block_lowers_to_raw_lua() {
    let source = r#"
pub fn patch() {
    unsafe {
        lua! {
            game.print("hello")
        }
    }
}
"#;

    let module = must_ok_parse(parse_module(source, "control"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
    };

    assert_eq!(function.body.statements.len(), 1);
    let Statement::RawLua { code } = &function.body.statements[0] else {
        panic!(
            "expected RawLua statement, got {:?}",
            function.body.statements[0]
        );
    };
    assert!(
        code.contains("game.print"),
        "code should contain game.print call, got: {code:?}"
    );
}

#[test]
fn lua_macro_outside_unsafe_errors() {
    let source = r#"
pub fn safe_fn() {
    lua! {
        local x = 1
    }
}
"#;

    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    let result = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    );
    assert!(result.is_err(), "expected error for lua! outside unsafe");
    let err = result.unwrap_err();
    assert!(
        matches!(err, FrontendError::LuaOutsideUnsafe { .. }),
        "expected LuaOutsideUnsafe error, got {err:?}"
    );
}

#[test]
fn lua_macro_codegen_emits_raw_lines() {
    let source = r#"
pub unsafe fn emit_raw() {
    lua! {
        local t = {}
        t.x = 42
    }
}
"#;

    let module = must_ok_parse(parse_module(source, "control"));
    let lua = LuaGenerator::new().generate_module(&module).unwrap();

    assert!(
        lua.contains("local t = {}"),
        "expected raw lua line in output:\n{lua}"
    );
    assert!(
        lua.contains("t.x = 42"),
        "expected raw lua line in output:\n{lua}"
    );
}

#[test]
fn lua_macro_blank_line_trimming() {
    let source = "pub unsafe fn f() { lua! {\n\n    game.print(\"hi\")\n\n} }";

    let module = must_ok_parse(parse_module(source, "control"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
    };

    let Statement::RawLua { code } = &function.body.statements[0] else {
        panic!("expected RawLua");
    };
    assert!(
        !code.starts_with('\n'),
        "leading blank line should be trimmed, got: {code:?}"
    );
    assert!(
        !code.ends_with('\n'),
        "trailing blank line should be trimmed, got: {code:?}"
    );
}
