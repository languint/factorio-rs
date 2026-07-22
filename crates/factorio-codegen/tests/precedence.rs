#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::missing_const_for_fn
)]

mod common;

use common::{assert_lua_fragment_parses, assert_lua_parses};
use factorio_codegen::LuaGenerator;
use factorio_ir::{expression::Expression, literal::Literal, operator::Operator};

fn id(name: &str) -> Expression {
    Expression::Identifier(name.to_string())
}

fn lit_int(n: i64) -> Expression {
    Expression::Literal(Literal::Int(n))
}

fn bin(lhs: Expression, op: Operator, rhs: Expression) -> Expression {
    Expression::BinaryOp {
        lhs: Box::new(lhs),
        op,
        rhs: Box::new(rhs),
    }
}

fn emit(expr: &Expression) -> String {
    let lua = LuaGenerator::new().generate_expression(expr);
    assert_lua_fragment_parses(&lua);
    lua
}

#[test]
fn mul_binds_tighter_than_add() {
    // a + b * c  and  a * b + c
    assert_eq!(
        emit(&bin(id("a"), Operator::Add, bin(id("b"), Operator::Mul, id("c")))),
        "a + b * c"
    );
    assert_eq!(
        emit(&bin(bin(id("a"), Operator::Mul, id("b")), Operator::Add, id("c"))),
        "a * b + c"
    );
}

#[test]
fn and_binds_tighter_than_or() {
    assert_eq!(
        emit(&bin(id("a"), Operator::Or, bin(id("b"), Operator::And, id("c")))),
        "a or b and c"
    );
    assert_eq!(
        emit(&bin(bin(id("a"), Operator::And, id("b")), Operator::Or, id("c"))),
        "a and b or c"
    );
}

#[test]
fn comparison_vs_and_needs_parens() {
    // (a == b) and c  vs  a == (b and c)
    assert_eq!(
        emit(&bin(
            bin(id("a"), Operator::Eq, id("b")),
            Operator::And,
            id("c")
        )),
        "a == b and c"
    );
    assert_eq!(
        emit(&bin(
            id("a"),
            Operator::Eq,
            bin(id("b"), Operator::And, id("c"))
        )),
        "a == (b and c)"
    );
}

#[test]
fn same_precedence_is_left_associative_with_rhs_parens() {
    // a - (b - c) must keep parens; a - b - c is fine left-assoc
    assert_eq!(
        emit(&bin(
            id("a"),
            Operator::Sub,
            bin(id("b"), Operator::Sub, id("c"))
        )),
        "a - (b - c)"
    );
    assert_eq!(
        emit(&bin(
            bin(id("a"), Operator::Sub, id("b")),
            Operator::Sub,
            id("c")
        )),
        "a - b - c"
    );
}

#[test]
fn unary_neg_from_zero_minus() {
    assert_eq!(
        emit(&bin(lit_int(0), Operator::Sub, id("x"))),
        "-x"
    );
    assert_eq!(
        emit(&bin(
            lit_int(0),
            Operator::Sub,
            bin(id("a"), Operator::Add, id("b"))
        )),
        "-(a + b)"
    );
    // Unary-neg currently parenthesizes whenever min_prec > 0 (lhs of `+`).
    assert_eq!(
        emit(&bin(
            bin(lit_int(0), Operator::Sub, id("x")),
            Operator::Add,
            id("y")
        )),
        "(-x) + y"
    );
    // a * -x needs parens around unary when min_prec is high
    assert_eq!(
        emit(&bin(
            id("a"),
            Operator::Mul,
            bin(lit_int(0), Operator::Sub, id("x"))
        )),
        "a * (-x)"
    );
}

#[test]
fn operator_matrix_emits_and_parses() {
    let ops = [
        Operator::Add,
        Operator::Sub,
        Operator::Mul,
        Operator::Div,
        Operator::Mod,
        Operator::Eq,
        Operator::Ne,
        Operator::Lt,
        Operator::Le,
        Operator::Gt,
        Operator::Ge,
        Operator::And,
        Operator::Or,
    ];
    for &outer in &ops {
        for &inner in &ops {
            let left_inner = bin(
                bin(id("a"), inner, id("b")),
                outer,
                id("c"),
            );
            let right_inner = bin(
                id("a"),
                outer,
                bin(id("b"), inner, id("c")),
            );
            let _ = emit(&left_inner);
            let _ = emit(&right_inner);
        }
    }
}

#[test]
fn format_concat_parses_as_fragment() {
    let expr = Expression::FormatConcat {
        parts: vec![
            Expression::Literal(Literal::String("a".to_string())),
            id("x"),
            Expression::Literal(Literal::String("b".to_string())),
        ],
    };
    let lua = emit(&expr);
    assert_eq!(lua, "\"a\" .. x .. \"b\"");
    assert_lua_parses(&format!("return ({lua})"));
}
