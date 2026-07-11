mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::{
    expression::Expression, literal::Literal, operator::Operator, statement::Statement,
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
        function
            .debug
            .as_ref()
            .and_then(|debug| debug.return_type.as_deref()),
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

#[test]
fn lowers_literal_union_enum_variant_to_string() {
    let source = r"
pub fn direction() -> &'static str {
    GuiDirection::Horizontal
}
";

    let module = must_ok_parse(parse_module(source, "control.gui"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function declaration");
        return;
    };

    assert_eq!(
        function.body.statements,
        vec![Statement::Return(Some(Expression::Literal(
            Literal::String("horizontal".to_string()),
        )))]
    );
}

#[test]
fn transpiles_literal_union_enum_variant_to_lua_string() {
    use factorio_codegen::LuaGenerator;

    let source = r"
pub fn direction() -> &'static str {
    GuiDirection::Vertical
}
";
    let module = must_ok_parse(parse_module(source, "control.gui"));
    let mut generator = LuaGenerator::new();
    let lua = generator.generate_module(&module).expect("generate");
    assert!(lua.contains("return \"vertical\""), "lua was:\n{lua}");
}

#[test]
fn lowers_some_constructor_to_inner_value() {
    use factorio_codegen::LuaGenerator;

    let source = r"
pub fn shout(message: &str) {
    game.print(
        message,
        Some(PrintSettings {
            color: Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
            ..Default::default()
        }),
    );
}
";

    let module = must_ok_parse(parse_module(source, "control.print_opt"));
    let lua = LuaGenerator::new()
        .generate_module(&module)
        .expect("generate");
    assert!(
        lua.contains("game.print(message, {"),
        "expected bare settings table, got:\n{lua}"
    );
    assert!(
        !lua.contains("Some("),
        "Some should not appear in Lua:\n{lua}"
    );
}

#[test]
fn lowers_let_chains_in_if_conditions() {
    let source = r"
pub fn check(flag: bool, value: Option<i32>) {
    if flag && let Some(x) = value {
        let y = x;
    }
}
";

    let module = must_ok_parse(parse_module(source, "control.let_chain"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function declaration");
        return;
    };

    assert_eq!(
        function.body.statements,
        vec![Statement::Conditional {
            condition: Expression::Identifier("flag".to_string()),
            then_block: vec![
                Statement::VariableDecl {
                    name: "x".to_string(),
                    ty: factorio_ir::r#type::Type::Void,
                    source_type: None,
                    value: Expression::Identifier("value".to_string()),
                },
                Statement::Conditional {
                    condition: Expression::Identifier("x".to_string()),
                    then_block: vec![Statement::VariableDecl {
                        name: "y".to_string(),
                        ty: factorio_ir::r#type::Type::Void,
                        source_type: None,
                        value: Expression::Identifier("x".to_string()),
                    }],
                    else_block: vec![],
                },
            ],
            else_block: vec![],
        }]
    );
}

#[test]
fn lowers_leading_if_let_in_chain() {
    let source = r"
pub fn check(value: Option<i32>) {
    if let Some(x) = value && x > 0 {
        let y = x;
    }
}
";

    let module = must_ok_parse(parse_module(source, "control.let_chain"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function declaration");
        return;
    };

    assert_eq!(
        &function.body.statements[0],
        &Statement::VariableDecl {
            name: "x".to_string(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: Expression::Identifier("value".to_string()),
        }
    );
    let Statement::Conditional {
        condition,
        then_block,
        ..
    } = &function.body.statements[1]
    else {
        assert_eq!(1, 0, "expected conditional after binding");
        return;
    };
    assert_eq!(condition, &Expression::Identifier("x".to_string()));
    let Statement::Conditional {
        condition: inner_cond,
        ..
    } = &then_block[0]
    else {
        assert_eq!(1, 0, "expected nested condition for `x > 0`");
        return;
    };
    assert!(matches!(
        inner_cond,
        Expression::BinaryOp {
            op: Operator::Gt,
            ..
        }
    ));
}
