mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::{
    expression::Expression,
    literal::Literal,
    operator::Operator,
    statement::Statement,
};

#[test]
fn parses_if_else_and_binary_ops() {
    let source = r"
pub fn add(a: i32, b: i32) -> i32 {
    if a == 0 {
        return b;
    } else {
        return a + b;
    }
}
";

    let module = must_ok_parse(parse_module(source, "control.math_util"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function declaration");
        return;
    };

    assert_eq!(function.name, "add");
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].source_type.as_deref(), Some("i32"));
    assert_eq!(
        function.debug.as_ref().and_then(|debug| debug.return_type.as_deref()),
        Some("i32")
    );
    assert_eq!(
        function.body.statements,
        vec![Statement::Conditional {
            condition: Expression::BinaryOp {
                lhs: Box::new(Expression::Identifier("a".to_string())),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::Int(0))),
            },
            then_block: vec![Statement::Return(Some(Expression::Identifier(
                "b".to_string(),
            )))],
            else_block: vec![Statement::Return(Some(Expression::BinaryOp {
                lhs: Box::new(Expression::Identifier("a".to_string())),
                op: Operator::Add,
                rhs: Box::new(Expression::Identifier("b".to_string())),
            }))],
        }]
    );
}

#[test]
fn parses_assignment() {
    let source = r"
pub fn bump(counter: i32) {
    counter = counter + 1;
}
";

    let module = must_ok_parse(parse_module(source, "control.counter"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function declaration");
        return;
    };

    assert_eq!(
        function.body.statements,
        vec![Statement::Assignment {
            target: Expression::Identifier("counter".to_string()),
            value: Expression::BinaryOp {
                lhs: Box::new(Expression::Identifier("counter".to_string())),
                op: Operator::Add,
                rhs: Box::new(Expression::Literal(Literal::Int(1))),
            },
        }]
    );
}

#[test]
fn parses_compound_assignment_and_comparisons() {
    let source = r"
pub fn damage(player: MyPlayer, amount: u64) {
    if player.health - amount > 0 {
        player.health -= amount;
    } else {
        player.health = 0;
    }
}
";

    let module = must_ok_parse(parse_module(source, "control.combat"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function declaration");
        return;
    };

    assert_eq!(function.params[0].source_type.as_deref(), Some("MyPlayer"));
    assert_eq!(function.params[1].source_type.as_deref(), Some("u64"));

    assert_eq!(
        function.body.statements,
        vec![Statement::Conditional {
            condition: Expression::BinaryOp {
                lhs: Box::new(Expression::BinaryOp {
                    lhs: Box::new(Expression::FieldAccess {
                        base: Box::new(Expression::Identifier("player".to_string())),
                        field: "health".to_string(),
                    }),
                    op: Operator::Sub,
                    rhs: Box::new(Expression::Identifier("amount".to_string())),
                }),
                op: Operator::Gt,
                rhs: Box::new(Expression::Literal(Literal::Int(0))),
            },
            then_block: vec![Statement::Assignment {
                target: Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("player".to_string())),
                    field: "health".to_string(),
                },
                value: Expression::BinaryOp {
                    lhs: Box::new(Expression::FieldAccess {
                        base: Box::new(Expression::Identifier("player".to_string())),
                        field: "health".to_string(),
                    }),
                    op: Operator::Sub,
                    rhs: Box::new(Expression::Identifier("amount".to_string())),
                },
            }],
            else_block: vec![Statement::Assignment {
                target: Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("player".to_string())),
                    field: "health".to_string(),
                },
                value: Expression::Literal(Literal::Int(0)),
            }],
        }]
    );
}
