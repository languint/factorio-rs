#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::{expression::Expression, statement::Statement};

#[test]
fn lowers_numeric_and_ordered_collection_loops() {
    let module = must_ok_parse(parse_module(
        r"
        pub fn loops(n: i64, values: Vec<i64>, other: String) {
            for i in 0..n {}
            for j in 0..=n {}
            for value in values {}
            for value in other {}
            for value in values.iter() {}
        }
        ",
        "control",
    ));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };

    assert!(matches!(
        &function.body.statements[0],
        Statement::ForNumeric {
            start: Expression::Literal(_),
            limit: Expression::BinaryOp { .. },
            ..
        }
    ));
    assert!(matches!(
        &function.body.statements[1],
        Statement::ForNumeric {
            limit: Expression::Identifier(limit),
            ..
        } if limit == "n"
    ));
    assert!(matches!(
        &function.body.statements[2],
        Statement::ForIn { ipairs: true, .. }
    ));
    assert!(matches!(
        &function.body.statements[3],
        Statement::ForIn { ipairs: false, .. }
    ));
    assert!(matches!(
        &function.body.statements[4],
        Statement::ForIn { ipairs: true, .. }
    ));
}

#[test]
fn lowers_map_and_filter_collect_chains_to_iifes() {
    let module = must_ok_parse(parse_module(
        r"
        pub fn collect(n: i64, values: Vec<i64>) {
            let mapped = (0..n).map(|i| i + 1).collect::<Vec<_>>();
            let filtered = (0..=n).filter(|i| i > 0).collect::<Vec<_>>();
            let both = values.iter().map(|i| i + 1).filter(|i| i > 1).collect::<Vec<_>>();
        }
        ",
        "control",
    ));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };

    for statement in &function.body.statements {
        let Statement::VariableDecl { value, .. } = statement else {
            panic!("expected collection binding");
        };
        assert!(matches!(
            value,
            Expression::Call { func, args }
                if args.is_empty() && matches!(func.as_ref(), Expression::Closure { .. })
        ));
    }
}

#[test]
fn rejects_unsupported_iterator_expressions() {
    for source in [
        "pub fn f(v: Vec<i64>) { let _x = v.iter(); }",
        "pub fn f(v: Vec<i64>) { let _x = v.collect::<Vec<_>>(); }",
        "pub fn f(v: Vec<i64>) { let _x = v.iter().enumerate().collect::<Vec<_>>(); }",
    ] {
        assert!(parse_module(source, "control").is_err());
    }
}
