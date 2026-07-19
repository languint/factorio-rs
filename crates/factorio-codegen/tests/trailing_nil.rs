mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    literal::Literal,
    module::{Module, Symbol},
    scope::Scope,
    stage::Stage,
    statement::Statement,
    r#type::Type,
};

#[test]
fn omit_trailing_nil_args_from_calls() {
    let module = Module {
        name: "control".to_string(),
        stage: Stage::Control,
        body: Block { statements: vec![] },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(Function {
                name: "die".to_string(),
                params: vec![Parameter {
                    name: "entity".to_string(),
                    r#type: Type::Void,
                    source_type: None,
                }],
                body: Block {
                    statements: vec![Statement::Expr(Expression::MethodCall {
                        receiver: Box::new(Expression::Identifier("entity".to_string())),
                        method: "die".to_string(),
                        args: vec![
                            Expression::Literal(Literal::Nil),
                            Expression::Literal(Literal::Nil),
                        ],
                    })],
                },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
            }),
        }],
    };

    let output = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(
        output.contains("entity.die()"),
        "trailing nils should become empty call, got:\n{output}"
    );
}

#[test]
fn keeps_non_trailing_nil_args() {
    let expr = Expression::Call {
        func: Box::new(Expression::Identifier("f".to_string())),
        args: vec![
            Expression::Literal(Literal::Nil),
            Expression::Literal(Literal::Int(1)),
            Expression::Literal(Literal::Nil),
        ],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "f(nil, 1)");
}

#[test]
fn storage_set_lowers_to_index_assignment() {
    let expr = Expression::MethodCall {
        receiver: Box::new(Expression::Identifier("storage".to_string())),
        method: "set".to_string(),
        args: vec![
            Expression::Literal(Literal::String("counter".to_string())),
            Expression::Literal(Literal::Int(1)),
        ],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "storage[\"counter\"] = 1");
}

#[test]
fn attribute_setter_lowers_to_property_assignment() {
    let expr = Expression::MethodCall {
        receiver: Box::new(Expression::Identifier("elem".to_string())),
        method: "set_caption".to_string(),
        args: vec![Expression::Literal(Literal::String("Hello".to_string()))],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "elem.caption = \"Hello\"");

    let style = Expression::MethodCall {
        receiver: Box::new(Expression::MethodCall {
            receiver: Box::new(Expression::Identifier("elem".to_string())),
            method: "style".to_string(),
            args: vec![],
        }),
        method: "set_width".to_string(),
        args: vec![Expression::Literal(Literal::Int(32))],
    };
    assert_eq!(
        LuaGenerator::new().generate_expression(&style),
        "elem.style.width = 32"
    );
}

#[test]
fn real_factorio_set_method_stays_method_call() {
    let expr = Expression::MethodCall {
        receiver: Box::new(Expression::Identifier("entity".to_string())),
        method: "set_filter".to_string(),
        args: vec![
            Expression::Literal(Literal::Int(1)),
            Expression::Literal(Literal::String("iron-ore".to_string())),
        ],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "entity.set_filter(1, \"iron-ore\")");
}

#[test]
fn storage_get_lowers_to_index_without_settings_value() {
    let expr = Expression::MethodCall {
        receiver: Box::new(Expression::Identifier("storage".to_string())),
        method: "get".to_string(),
        args: vec![Expression::Literal(Literal::String("counter".to_string()))],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "storage[\"counter\"]");
}

#[test]
fn settings_get_still_appends_value() {
    let expr = Expression::MethodCall {
        receiver: Box::new(Expression::FieldAccess {
            base: Box::new(Expression::Identifier("settings".to_string())),
            field: "startup".to_string(),
        }),
        method: "get".to_string(),
        args: vec![Expression::Literal(Literal::String(
            "ms-casual-mode".to_string(),
        ))],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "settings.startup[\"ms-casual-mode\"].value");
}

#[test]
fn generates_safe_if_expression() {
    let expr = Expression::If {
        condition: Box::new(Expression::Identifier("cond".to_string())),
        then_expr: Box::new(Expression::Literal(Literal::Int(0))),
        else_expr: Box::new(Expression::Literal(Literal::Int(1))),
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(
        lua,
        "(function() if cond then return 0 else return 1 end end)()"
    );
}

#[test]
fn generates_unwrap_or_preserving_falsey_some() {
    // `x.unwrap_or(true)` when x is `false` must return `false`, not the default.
    let expr = Expression::If {
        condition: Box::new(Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("x".to_string())),
            op: factorio_ir::operator::Operator::Ne,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }),
        then_expr: Box::new(Expression::Identifier("x".to_string())),
        else_expr: Box::new(Expression::Literal(Literal::Bool(true))),
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(
        lua,
        "(function() if x ~= nil then return x else return true end end)()"
    );
}

#[test]
fn generates_closure_and_option_map() {
    use factorio_ir::block::Block;
    use factorio_ir::operator::Operator;

    let closure = Expression::Closure {
        params: vec!["n".to_string()],
        body: Block {
            statements: vec![Statement::Return(Some(Expression::BinaryOp {
                lhs: Box::new(Expression::Identifier("n".to_string())),
                op: Operator::Add,
                rhs: Box::new(Expression::Literal(Literal::Int(1))),
            }))],
        },
    };
    let lua = LuaGenerator::new().generate_expression(&closure);
    assert_eq!(lua, "function(n) return n + 1 end");

    let map = Expression::If {
        condition: Box::new(Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("x".to_string())),
            op: Operator::Ne,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }),
        then_expr: Box::new(Expression::Call {
            func: Box::new(closure),
            args: vec![Expression::Identifier("x".to_string())],
        }),
        else_expr: Box::new(Expression::Literal(Literal::Nil)),
    };
    let lua = LuaGenerator::new().generate_expression(&map);
    assert!(lua.contains("x ~= nil"), "{lua}");
    assert!(lua.contains("function(n) return n + 1 end"), "{lua}");
    assert!(
        lua.contains("(function(n) return n + 1 end)(x)") || lua.contains("return (function"),
        "{lua}"
    );
}

#[test]
fn omits_nil_fields_from_struct_literals() {
    let expr = Expression::StructLiteral {
        struct_name: Some("PrintSettings".to_string()),
        fields: vec![
            ("color".to_string(), Expression::Identifier("c".to_string())),
            ("skip".to_string(), Expression::Literal(Literal::Nil)),
        ],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "{ color = c }");
}
