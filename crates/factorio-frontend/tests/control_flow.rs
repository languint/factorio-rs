#![allow(
    clippy::expect_used,
    clippy::literal_string_with_formatting_args,
    clippy::needless_raw_string_hashes,
    clippy::panic,
    clippy::unwrap_used
)]
mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::{
    expression::Expression, literal::Literal, operator::Operator, statement::Statement,
};

#[test]
fn parses_while_continue_and_break() {
    let source = r"
pub fn tick(mut n: i32) {
    while n > 0 {
        if n == 1 {
            break;
        }
        n = n - 1;
        continue;
    }
}
";

    let module = must_ok_parse(parse_module(source, "control.loops"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
    };

    assert_eq!(
        function.body.statements,
        vec![Statement::While {
            condition: Expression::BinaryOp {
                lhs: Box::new(Expression::Identifier("n".to_string())),
                op: Operator::Gt,
                rhs: Box::new(Expression::Literal(Literal::Int(0))),
            },
            body: vec![
                Statement::Conditional {
                    condition: Expression::BinaryOp {
                        lhs: Box::new(Expression::Identifier("n".to_string())),
                        op: Operator::Eq,
                        rhs: Box::new(Expression::Literal(Literal::Int(1))),
                    },
                    then_block: vec![Statement::Break],
                    else_block: vec![],
                },
                Statement::Assignment {
                    target: Expression::Identifier("n".to_string()),
                    value: Expression::BinaryOp {
                        lhs: Box::new(Expression::Identifier("n".to_string())),
                        op: Operator::Sub,
                        rhs: Box::new(Expression::Literal(Literal::Int(1))),
                    },
                },
                Statement::Continue,
            ],
        }]
    );
}

#[test]
fn parses_loop_as_while_true() {
    let source = r"
pub fn forever() {
    loop {
        break;
    }
}
";

    let module = must_ok_parse(parse_module(source, "control.loops"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
    };

    assert_eq!(
        function.body.statements,
        vec![Statement::While {
            condition: Expression::Literal(Literal::Bool(true)),
            body: vec![Statement::Break],
        }]
    );
}

#[test]
fn parses_statement_match_option() {
    let source = r"
pub fn handle(value: Option<i32>) {
    match value {
        Some(x) => {
            return x;
        }
        None => {
            return 0;
        }
    };
}
";

    let module = must_ok_parse(parse_module(source, "control.match_stmt"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
    };

    assert_eq!(function.body.statements.len(), 2);
    let Statement::VariableDecl { name, value, .. } = &function.body.statements[0] else {
        panic!("expected match temp");
    };
    assert!(name.starts_with("__match_"));
    assert_eq!(value, &Expression::Identifier("value".to_string()));

    let Statement::Conditional {
        condition,
        then_block,
        else_block,
    } = &function.body.statements[1]
    else {
        panic!("expected match conditional");
    };
    assert_eq!(
        condition,
        &Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier(name.clone())),
            op: Operator::Ne,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }
    );
    assert!(matches!(
        then_block.as_slice(),
        [
            Statement::VariableDecl { .. },
            Statement::Return(Some(Expression::Identifier(_)))
        ]
    ));
    let Statement::Conditional {
        condition: none_cond,
        then_block: none_then,
        ..
    } = &else_block[0]
    else {
        panic!("expected None arm");
    };
    assert_eq!(
        none_cond,
        &Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier(name.clone())),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }
    );
    assert_eq!(
        none_then,
        &vec![Statement::Return(Some(Expression::Literal(Literal::Int(
            0
        ))))]
    );
}

#[test]
fn parses_value_match_as_statements() {
    let source = r"
pub fn classify(flag: bool) -> i32 {
    match flag {
        true => 1,
        false => 0,
    }
}
";

    let module = must_ok_parse(parse_module(source, "control.match_val"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
    };

    assert!(matches!(
        function.body.statements.as_slice(),
        [
            Statement::VariableDecl { .. },
            Statement::Conditional { .. }
        ]
    ));
}

#[test]
fn parses_match_guard() {
    let source = r"
pub fn handle(value: Option<i32>) {
    match value {
        Some(x) if x > 0 => {
            return x;
        }
        _ => {
            return 0;
        }
    };
}
";

    let module = must_ok_parse(parse_module(source, "control.match_guard"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::VariableDecl { .. } = &function.body.statements[0] else {
        panic!("expected match temp");
    };
    let Statement::Conditional {
        then_block,
        else_block,
        ..
    } = &function.body.statements[1]
    else {
        panic!("expected Some arm");
    };
    // Some arm: bind x, then nested `if x > 0` with fallthrough else.
    assert!(matches!(
        then_block.as_slice(),
        [
            Statement::VariableDecl { .. },
            Statement::Conditional {
                condition: Expression::BinaryOp {
                    op: Operator::Gt,
                    ..
                },
                ..
            }
        ]
    ));
    let Statement::Conditional {
        else_block: guard_else,
        ..
    } = &then_block[1]
    else {
        panic!("expected guard conditional");
    };
    // Irrefutable `_` fallthrough is identical for guard-fail and pattern-miss.
    assert_eq!(guard_else, else_block);
}

#[test]
fn guarded_enum_arms_do_not_duplicate_later_tag_tests() {
    let source = r"
pub enum Phase {
    Idle,
    Mining { ticks: i64 },
    Done,
}

impl Phase {
    pub fn tick(self) -> Phase {
        match self {
            Phase::Idle => Phase::Mining { ticks: 0 },
            Phase::Mining { ticks } if ticks + 1 >= 60 => Phase::Done,
            Phase::Mining { ticks } => Phase::Mining { ticks: ticks + 1 },
            Phase::Done => Phase::Done,
        }
    }
}
";

    let mut module = must_ok_parse(parse_module(source, "shared.phase"));
    factorio_ir::opt::optimize_modules(std::slice::from_mut(&mut module));
    let lua = factorio_codegen::LuaGenerator::new()
        .generate_module(&module)
        .expect("generate");

    let tag_rawgets = lua.matches("rawget(self, \"tag\")").count();
    assert_eq!(
        tag_rawgets, 1,
        "tag should be rawget'd once into a local, got {tag_rawgets} in:\n{lua}"
    );
    let ticks_rawgets = lua.matches("rawget(self, \"ticks\")").count();
    assert_eq!(
        ticks_rawgets, 1,
        "ticks binding should not be repeated on guard fallthrough, got {ticks_rawgets} in:\n{lua}"
    );
    assert!(
        lua.contains("ticks + 1 >= 60")
            || (lua.contains("ticks = ticks + 1") && lua.contains("ticks >= 60")),
        "expected guard as `ticks + 1 >= 60` or peephole mutate-local form:\n{lua}"
    );
    assert_eq!(
        lua.matches("ticks + 1").count(),
        1,
        "ticks + 1 should appear once (guard or mutate), got:\n{lua}"
    );
    let mining_checks = lua.matches("== \"Mining\"").count();
    assert_eq!(
        mining_checks, 1,
        "expected a single Mining discriminant test, got {mining_checks} in:\n{lua}"
    );
}

#[test]
fn parses_match_or_pattern() {
    let source = r"
pub fn classify(n: i32) -> i32 {
    match n {
        1 | 2 => 10,
        _ => 0,
    }
}
";

    let module = must_ok_parse(parse_module(source, "control.match_or"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let body = &function.body.statements;
    let Statement::VariableDecl { name, .. } = &body[0] else {
        panic!("expected temp");
    };
    // `1 | 2` expands to nested arms: if == 1 then ... else if == 2 then ... else _
    let Statement::Conditional {
        condition,
        else_block,
        ..
    } = &body[1]
    else {
        panic!("expected first or-alt");
    };
    assert_eq!(
        condition,
        &Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier(name.clone())),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Int(1))),
        }
    );
    let Statement::Conditional {
        condition: cond2, ..
    } = &else_block[0]
    else {
        panic!("expected second or-alt");
    };
    assert_eq!(
        cond2,
        &Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier(name.clone())),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Int(2))),
        }
    );
}

#[test]
fn parses_match_struct_pattern() {
    let source = r#"
pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub fn origin_x(p: Point) -> i32 {
    match p {
        Point { x, y: 0, .. } => x,
        Point { x, y, .. } => x + y,
    }
}
"#;

    let module = must_ok_parse(parse_module(source, "control.match_struct"));
    let function = module
        .symbols
        .iter()
        .find_map(|s| match &s.statement {
            Statement::FunctionDecl(f) if f.name == "origin_x" => Some(f),
            _ => None,
        })
        .expect("origin_x");

    let body = &function.body.statements;
    let Statement::VariableDecl { name, .. } = &body[0] else {
        panic!("expected temp");
    };
    let Statement::Conditional {
        condition,
        then_block,
        ..
    } = &body[1]
    else {
        panic!("expected y: 0 arm");
    };
    assert_eq!(
        condition,
        &Expression::BinaryOp {
            lhs: Box::new(Expression::FieldAccess {
                base: Box::new(Expression::Identifier(name.clone())),
                field: "y".to_string(),
            }),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Int(0))),
        }
    );
    assert!(matches!(
        then_block.as_slice(),
        [
            Statement::VariableDecl {
                name: x_name,
                value: Expression::FieldAccess { field, .. },
                ..
            },
            Statement::Return(Some(Expression::Identifier(ret)))
        ] if x_name == "x" && field == "x" && ret == "x"
    ));
}
