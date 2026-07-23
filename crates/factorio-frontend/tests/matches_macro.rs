#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::{expression::Expression, statement::Statement};

#[test]
fn lowers_matches_macro_for_enum_and_option() {
    let module = must_ok_parse(parse_module(
        r"
        pub enum Msg {
            Quit,
            Move(i64),
        }

        impl Msg {
            pub fn is_quit(&self) -> bool {
                matches!(self, Self::Quit)
            }
        }

        pub fn check(msg: Msg, opt: Option<i64>) -> bool {
            let a = matches!(msg, Msg::Move(_));
            let b = matches!(opt, Some(n) if n > 0);
            let c = matches!(opt, None | Some(0));
            a && b && c
        }
        ",
        "shared.matches",
    ));

    let check = module
        .symbols
        .iter()
        .find_map(|symbol| match &symbol.statement {
            Statement::FunctionDecl(function) if function.name == "check" => Some(function),
            _ => None,
        })
        .expect("expected check function");

    let conditionals = check
        .body
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::Conditional { .. }))
        .count();
    assert!(
        conditionals >= 3,
        "matches! should emit statement conditionals, got {:?}",
        check.body.statements
    );

    let bindings = ["a", "b", "c"];
    for name in bindings {
        assert!(
            check.body.statements.iter().any(|s| matches!(
                s,
                Statement::VariableDecl {
                    name: n,
                    value: Expression::Identifier(tmp),
                    ..
                } if n == name && tmp.starts_with("__match_")
            )),
            "expected `{name}` bound to a match result temp, got {:?}",
            check.body.statements
        );
    }
}

#[test]
fn rejects_malformed_matches_macro() {
    assert!(parse_module("pub fn f(x: i64) -> bool { matches!(x) }", "control").is_err());
}
