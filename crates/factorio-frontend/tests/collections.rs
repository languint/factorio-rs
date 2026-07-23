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
            let odds = (0..=n).iter().filter(|i| *i % 2 == 1).collect::<Vec<_>>();
            let taken = (0..=n).filter(|i| *i % 2 == 1).take(5).collect::<Vec<_>>();
            let first = values.iter().take(3).collect::<Vec<_>>();
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

    // `(0..=n).iter().filter(...).collect()` must be a numeric for, not ipairs on a Range.
    let Statement::VariableDecl {
        value: Expression::Call { func, .. },
        ..
    } = &function.body.statements[3]
    else {
        panic!("expected odds IIFE");
    };
    let Expression::Closure { body, .. } = func.as_ref() else {
        panic!("expected closure");
    };
    assert!(
        body.statements
            .iter()
            .any(|s| matches!(s, Statement::ForNumeric { .. })),
        "range.iter() collect should use numeric for, got {body:?}"
    );

    let Statement::VariableDecl {
        value: Expression::Call { func, .. },
        ..
    } = &function.body.statements[4]
    else {
        panic!("expected take IIFE");
    };
    let Expression::Closure { body, .. } = func.as_ref() else {
        panic!("expected closure");
    };
    assert!(
        body.statements.iter().any(|s| matches!(
            s,
            Statement::VariableDecl { name, .. } if name.starts_with("__take_")
        )),
        "take should init a counter, got {body:?}"
    );
    assert!(
        body_contains_break(&body.statements),
        "take should break after the limit, got {body:?}"
    );
}

fn body_contains_break(stmts: &[Statement]) -> bool {
    stmts.iter().any(|s| match s {
        Statement::Break => true,
        Statement::Conditional {
            then_block,
            else_block,
            ..
        } => body_contains_break(then_block) || body_contains_break(else_block),
        Statement::ForIn { body, .. }
        | Statement::ForNumeric { body, .. }
        | Statement::While { body, .. } => body_contains_break(body),
        _ => false,
    })
}

#[test]
fn release_opt_tightens_map_filter_collect_bindings() {
    let mut module = must_ok_parse(parse_module(
        r"
        pub fn top_half(scores: Vec<i64>) -> Vec<i64> {
            scores.iter().filter(|s| *s >= 50).collect::<Vec<_>>()
        }

        pub fn indices(n: i64) -> Vec<i64> {
            (0..n).map(|i| i + 1).collect::<Vec<_>>()
        }

        pub fn on_init() {
            let scores = (0..5).map(|i| i * 20).collect::<Vec<_>>();
            let xs = top_half(scores);
            let ys = indices(3);
            let _n = xs.len() + ys.len();
        }
        ",
        "control",
    ));
    factorio_ir::opt::optimize_modules(std::slice::from_mut(&mut module));
    let lua = factorio_codegen::LuaGenerator::new()
        .generate_module(&module)
        .expect("generate");

    assert!(
        !lua.contains("__iter_value"),
        "map/filter should not need __iter_value after temp folds, got:\n{lua}"
    );
    assert!(
        !lua.contains("local scores = nil"),
        "let-bound collect should bind scores directly, got:\n{lua}"
    );
    assert!(
        lua.contains("local scores = {}"),
        "expected scores = {{}}, got:\n{lua}"
    );
    assert!(
        lua.contains("scores[#scores + 1] = __iter_item * 20"),
        "expected mapped push into scores, got:\n{lua}"
    );
    assert!(
        lua.contains("__out[#__out + 1] = __iter_item + 1"),
        "return-position map collect may keep __out but should rematerialize map, got:\n{lua}"
    );
}

/// End-to-end emit check for rewrite experiments: `% 2` must survive
/// `optimize_modules` (Factorio bench found `bit32.band` slower).
#[test]
fn release_opt_preserves_mod2_parity_in_for_body() {
    let mut module = must_ok_parse(parse_module(
        r"
        pub fn count_odds(limit: u32) -> u32 {
            let mut sink = 0_u32;
            for n in 0..limit {
                if n % 2 == 1 {
                    sink += 1;
                }
            }
            sink
        }
        ",
        "control",
    ));
    factorio_ir::opt::optimize_modules(std::slice::from_mut(&mut module));
    let lua = factorio_codegen::LuaGenerator::new()
        .generate_module(&module)
        .expect("generate");
    assert!(
        lua.contains("% 2"),
        "expected literal % 2 to remain after optimize, got:\n{lua}"
    );
    assert!(
        !lua.contains("bit32.band"),
        "bit32.band rewrite was reverted (slower in Factorio); got:\n{lua}"
    );
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
