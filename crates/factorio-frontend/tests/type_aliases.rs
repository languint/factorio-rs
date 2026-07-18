#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::{statement::Statement, r#type::Type};

#[test]
fn resolves_type_alias_on_bindings_and_params() {
    let module = must_ok_parse(parse_module(
        r"
        type Count = i64;
        type Entities = Vec<i64>;
        type Opt<T> = Option<T>;

        pub fn use_aliases(n: Count, values: Entities, maybe: Opt<i64>) {
            let total: Count = n;
            for value in values {}
            if let Some(x) = maybe {
                let _ = x;
            }
        }
        ",
        "control",
    ));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };

    assert_eq!(function.params[0].source_type.as_deref(), Some("i64"));
    assert_eq!(function.params[1].source_type.as_deref(), Some("Vec"));
    assert_eq!(function.params[2].source_type.as_deref(), Some("Option"));

    let Statement::VariableDecl {
        ty, source_type, ..
    } = &function.body.statements[0]
    else {
        panic!("expected total binding");
    };
    assert_eq!(*ty, Type::Int);
    assert_eq!(source_type.as_deref(), Some("i64"));

    assert!(matches!(
        &function.body.statements[1],
        Statement::ForIn { ipairs: true, .. }
    ));
}

#[test]
fn resolves_nested_and_generic_aliases() {
    let module = must_ok_parse(parse_module(
        r"
        type Inner = i32;
        type Outer = Inner;
        type BoxOpt<T> = Option<T>;

        pub fn nested(a: Outer, b: BoxOpt<i64>) -> Outer {
            let x: Outer = a;
            let _y: BoxOpt<i64> = b;
            x
        }
        ",
        "control",
    ));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };

    assert_eq!(
        function
            .debug
            .as_ref()
            .and_then(|debug| debug.return_type.as_deref()),
        Some("i32")
    );
    assert_eq!(function.params[0].source_type.as_deref(), Some("i32"));
    assert_eq!(function.params[1].source_type.as_deref(), Some("Option"));

    let Statement::VariableDecl {
        ty, source_type, ..
    } = &function.body.statements[0]
    else {
        panic!("expected x binding");
    };
    assert_eq!(*ty, Type::Int);
    assert_eq!(source_type.as_deref(), Some("i32"));
}

#[test]
fn local_type_alias_in_block_is_visible() {
    let module = must_ok_parse(parse_module(
        r"
        pub fn local_alias() {
            type Local = i64;
            let n: Local = 1;
        }
        ",
        "control",
    ));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::VariableDecl {
        ty, source_type, ..
    } = &function.body.statements[0]
    else {
        panic!("expected n binding");
    };
    assert_eq!(*ty, Type::Int);
    assert_eq!(source_type.as_deref(), Some("i64"));
}

#[test]
fn rejects_unsupported_type_alias_forms() {
    for source in [
        "type Bad<'a> = &'a str;",
        "type Bad<const N: usize> = i64;",
        "type Bad<T: Clone> = T;",
        "type Bad<T> where T: Clone = T;",
    ] {
        assert!(
            parse_module(source, "control").is_err(),
            "expected reject for {source}"
        );
    }
}

#[test]
fn type_alias_emits_no_ir_statement() {
    let module = must_ok_parse(parse_module(
        r"
        type Count = i64;
        pub fn f(n: Count) -> Count { n }
        ",
        "control",
    ));
    assert!(module.body.statements.is_empty());
    assert_eq!(module.symbols.len(), 1);
}
