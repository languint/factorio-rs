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
            color: Some(Color {
                r: Some(1.0),
                g: Some(0.0),
                b: Some(0.0),
                a: Some(1.0),
            }),
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
                    condition: Expression::BinaryOp {
                        lhs: Box::new(Expression::Identifier("x".to_string())),
                        op: Operator::Ne,
                        rhs: Box::new(Expression::Literal(Literal::Nil)),
                    },
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
    assert_eq!(
        condition,
        &Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("x".to_string())),
            op: Operator::Ne,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }
    );
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

#[test]
fn remaps_lua_library_overload_method_names() {
    let source = r"
pub fn sample(n: i64) -> i64 {
    math.random_int(n)
}
";

    let module = must_ok_parse(parse_module(source, "control.lua_libs"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        assert_eq!(1, 0, "expected function declaration");
        return;
    };

    let Statement::Return(Some(Expression::MethodCall { method, args, .. })) =
        &function.body.statements[0]
    else {
        assert_eq!(1, 0, "expected return of method call");
        return;
    };

    assert_eq!(method, "random");
    assert_eq!(args.len(), 1);
}

#[cfg(feature = "serde")]
fn return_expr(module: &factorio_ir::module::Module) -> &Expression {
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
    };
    let Statement::Return(Some(expr)) = &function.body.statements[0] else {
        panic!("expected return expression");
    };
    expr
}

#[test]
#[cfg(feature = "serde")]
fn lowers_serde_json_to_string_to_helpers_table_to_json() {
    let source = r#"
pub fn encode(data: i64) -> &'static str {
    serde_json::to_string(&data).unwrap()
}
"#;
    let module = must_ok_parse(parse_module(source, "control.serde_json"));
    let Expression::MethodCall {
        receiver,
        method,
        args,
        ..
    } = return_expr(&module)
    else {
        panic!("expected method call");
    };
    assert_eq!(method, "table_to_json");
    assert_eq!(
        receiver.as_ref(),
        &Expression::Identifier("helpers".to_string())
    );
    assert_eq!(args, &[Expression::Identifier("data".to_string())]);
}

#[test]
#[cfg(feature = "serde")]
fn lowers_serde_json_from_str_to_helpers_json_to_table() {
    let source = r#"
pub fn decode(s: &'static str) -> i64 {
    serde_json::from_str::<i64>(s).unwrap()
}
"#;
    let module = must_ok_parse(parse_module(source, "control.serde_json"));
    let Expression::MethodCall {
        receiver,
        method,
        args,
        ..
    } = return_expr(&module)
    else {
        panic!("expected method call");
    };
    assert_eq!(method, "json_to_table");
    assert_eq!(
        receiver.as_ref(),
        &Expression::Identifier("helpers".to_string())
    );
    assert_eq!(args, &[Expression::Identifier("s".to_string())]);
}

#[test]
#[cfg(feature = "serde")]
fn lowers_serde_json_to_value_as_identity() {
    let source = r#"
pub fn as_value(data: i64) -> i64 {
    serde_json::to_value(data).unwrap()
}
"#;
    let module = must_ok_parse(parse_module(source, "control.serde_json"));
    assert_eq!(
        return_expr(&module),
        &Expression::Identifier("data".to_string())
    );
}

#[test]
#[cfg(feature = "serde")]
fn lowers_serde_json_to_vec_via_string_pack() {
    let source = r#"
pub fn encode_bin(data: i64) -> &'static str {
    serde_json::to_vec(&data).unwrap()
}
"#;
    let module = must_ok_parse(parse_module(source, "control.serde_json"));
    let Expression::MethodCall {
        receiver,
        method,
        args,
        ..
    } = return_expr(&module)
    else {
        panic!("expected string.pack call");
    };
    assert_eq!(method, "pack");
    assert_eq!(
        receiver.as_ref(),
        &Expression::Identifier("string".to_string())
    );
    assert_eq!(args.len(), 2);
    assert_eq!(
        &args[0],
        &Expression::Literal(Literal::String("s".to_string()))
    );
    let Expression::MethodCall {
        method: inner_method,
        ..
    } = &args[1]
    else {
        panic!("expected nested table_to_json");
    };
    assert_eq!(inner_method, "table_to_json");
}

#[test]
#[cfg(feature = "serde")]
fn lowers_serde_json_from_slice_via_string_unpack() {
    let source = r#"
pub fn decode_bin(blob: &'static str) -> i64 {
    serde_json::from_slice(blob).unwrap()
}
"#;
    let module = must_ok_parse(parse_module(source, "control.serde_json"));
    let Expression::MethodCall { method, args, .. } = return_expr(&module) else {
        panic!("expected json_to_table call");
    };
    assert_eq!(method, "json_to_table");
    let Expression::MethodCall {
        receiver,
        method: unpack,
        args: unpack_args,
        ..
    } = &args[0]
    else {
        panic!("expected string.unpack");
    };
    assert_eq!(unpack, "unpack");
    assert_eq!(
        receiver.as_ref(),
        &Expression::Identifier("string".to_string())
    );
    assert_eq!(
        &unpack_args[0],
        &Expression::Literal(Literal::String("s".to_string()))
    );
}

#[test]
#[cfg(feature = "serde")]
fn serde_json_roundtrip_emits_helpers_and_pack_lua() {
    let source = r#"
pub fn roundtrip(data: i64, blob: &'static str) -> i64 {
    let _s = serde_json::to_string(&data).unwrap();
    let _b = serde_json::to_vec(&data).unwrap();
    serde_json::from_slice(blob).unwrap()
}
"#;
    let module = must_ok_parse(parse_module(source, "control.serde_json"));
    let lua = factorio_codegen::LuaGenerator::new()
        .generate_module(&module)
        .expect("codegen");
    assert!(lua.contains("helpers.table_to_json(data)"));
    assert!(lua.contains("string.pack(\"s\", helpers.table_to_json(data))"));
    assert!(lua.contains("helpers.json_to_table(string.unpack(\"s\", blob))"));
}

#[test]
#[cfg(feature = "serde")]
fn rejects_unsupported_serde_json_macro() {
    let source = r#"
pub fn bad() {
    let _ = serde_json::json!({ "a": 1 });
}
"#;
    let err = parse_module(source, "control.serde_json").expect_err("json! unsupported");
    let msg = err.to_string();
    assert!(msg.contains("serde_json::json"), "{msg}");
}

fn ne_nil_ident(name: &str) -> Expression {
    Expression::BinaryOp {
        lhs: Box::new(Expression::Identifier(name.to_string())),
        op: Operator::Ne,
        rhs: Box::new(Expression::Literal(Literal::Nil)),
    }
}

#[test]
fn lowers_option_is_some_and_is_none() {
    let source = r"
pub fn check(x: Option<i32>) -> bool {
    x.is_some()
}
";
    let module = must_ok_parse(parse_module(source, "control.option_methods"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::Return(Some(expr)) = &function.body.statements[0] else {
        panic!("expected return, got {:?}", function.body.statements);
    };
    assert_eq!(expr, &ne_nil_ident("x"));

    let source = r"
pub fn check(x: Option<i32>) -> bool {
    x.is_none()
}
";
    let module = must_ok_parse(parse_module(source, "control.option_methods"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::Return(Some(expr)) = &function.body.statements[0] else {
        panic!("expected return");
    };
    assert_eq!(
        expr,
        &Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("x".to_string())),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }
    );
}

#[test]
fn lowers_option_unwrap_or_or_and() {
    let source = r"
pub fn check(x: Option<i32>, y: Option<i32>) -> i32 {
    let a = x.unwrap_or(0);
    let b = x.or(y);
    x.and(y)
}
";
    let module = must_ok_parse(parse_module(source, "control.option_methods"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };

    let body = &function.body.statements;
    let conditionals = body
        .iter()
        .filter(|s| matches!(s, Statement::Conditional { .. }))
        .count();
    assert!(
        conditionals >= 3,
        "expected hoisted unwrap_or/or/and conditionals, got {body:?}"
    );
    assert!(
        body.iter().any(|s| matches!(
            s,
            Statement::VariableDecl {
                name,
                value: Expression::Identifier(tmp),
                ..
            } if name == "a" && tmp.starts_with("__h_")
        )),
        "unwrap_or should bind a hoist temp, got {body:?}"
    );
}

#[test]
fn lowers_option_map_and_unwrap_or_else() {
    let source = r"
pub fn mapped(x: Option<i32>) -> Option<i32> {
    x.map(|n| n + 1)
}
";
    let module = must_ok_parse(parse_module(source, "control.option_methods"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let body = &function.body.statements;
    let Some(Statement::Conditional {
        condition,
        then_block,
        else_block,
        ..
    }) = body
        .iter()
        .find(|s| matches!(s, Statement::Conditional { .. }))
    else {
        panic!("expected hoisted map conditional, got {body:?}");
    };
    assert_eq!(condition, &ne_nil_ident("x"));
    assert!(matches!(
        then_block.as_slice(),
        [Statement::Assignment {
            value: Expression::Call { .. },
            ..
        }]
    ));
    assert!(matches!(
        else_block.as_slice(),
        [Statement::Assignment {
            value: Expression::Literal(Literal::Nil),
            ..
        }]
    ));

    let source = r"
pub fn or_else(x: Option<i32>) -> i32 {
    x.unwrap_or_else(|| 0)
}
";
    let module = must_ok_parse(parse_module(source, "control.option_methods"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let body = &function.body.statements;
    let Some(Statement::Conditional {
        then_block,
        else_block,
        ..
    }) = body
        .iter()
        .find(|s| matches!(s, Statement::Conditional { .. }))
    else {
        panic!("expected hoisted unwrap_or_else, got {body:?}");
    };
    assert!(matches!(
        then_block.as_slice(),
        [Statement::Assignment {
            value: Expression::Identifier(name),
            ..
        }] if name == "x"
    ));
    assert!(matches!(
        else_block.as_slice(),
        [Statement::Assignment {
            value: Expression::Call { args, .. },
            ..
        }] if args.is_empty()
    ));
}

#[test]
fn lowers_option_filter() {
    let source = r"
pub fn keep(x: Option<i32>) -> Option<i32> {
    x.filter(|n| n > 0)
}
";
    let module = must_ok_parse(parse_module(source, "control.option_methods"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let body = &function.body.statements;
    let Some(Statement::Conditional {
        condition,
        then_block,
        else_block,
        ..
    }) = body
        .iter()
        .find(|s| matches!(s, Statement::Conditional { .. }))
    else {
        panic!("filter should hoist a conditional, got {body:?}");
    };
    assert!(
        matches!(
            condition,
            Expression::BinaryOp {
                op: Operator::And,
                ..
            }
        ),
        "filter should use `~= nil and pred`, got {condition:?}"
    );
    assert!(
        matches!(
            then_block.as_slice(),
            [Statement::Assignment {
                value: Expression::Identifier(name),
                ..
            }] if name == "x"
        ),
        "then should keep x, got {then_block:?}"
    );
    assert!(
        else_block.is_empty(),
        "else should be empty (result starts nil), got {else_block:?}"
    );
    assert!(matches!(
        body.last(),
        Some(Statement::Return(Some(Expression::Identifier(_))))
    ));
}

#[test]
fn rejects_unwrap_or_default() {
    let source = r"
pub fn f(x: Option<i32>) -> i32 {
    x.unwrap_or_default()
}
";
    let err = parse_module(source, "control.option_default").expect_err("unwrap_or_default");
    let msg = format!("{err}");
    assert!(
        msg.contains("unwrap_or_default") || msg.contains("Unsupported"),
        "expected unwrap_or_default error, got {msg}"
    );
}

#[test]
fn rejects_unsupported_result_method() {
    let source = r#"
pub fn f(r: Result<i32, String>) -> i32 {
    r.unwrap_err()
}
"#;
    let err = parse_module(source, "control.result_unsupported").expect_err("unwrap_err");
    let msg = format!("{err}");
    assert!(
        msg.contains("unwrap_err") || msg.contains("not supported"),
        "expected unsupported method error, got {msg}"
    );
}

#[test]
fn lowers_closures_expr_and_block() {
    let source = r"
pub fn make() {
    let f = |n| n + 1;
    let g = |n| {
        let m = n + 1;
        m
    };
}
";
    let module = must_ok_parse(parse_module(source, "control.closures"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::VariableDecl { value: f, .. } = &function.body.statements[0] else {
        panic!("expected f");
    };
    let Expression::Closure { params, body } = f else {
        panic!("expected closure, got {f:?}");
    };
    assert_eq!(params, &["n".to_string()]);
    assert!(matches!(
        body.statements.as_slice(),
        [Statement::Return(Some(Expression::BinaryOp { .. }))]
    ));

    let Statement::VariableDecl { value: g, .. } = &function.body.statements[1] else {
        panic!("expected g");
    };
    let Expression::Closure { body, .. } = g else {
        panic!("expected block closure");
    };
    assert!(body.statements.len() >= 2);
}

#[test]
fn rejects_async_and_tuple_closure_params() {
    let source = r"
pub fn bad() {
    let _f = async || 1;
}
";
    let err = parse_module(source, "control.closures").expect_err("async");
    assert!(
        err.to_string().contains("unsupported") || err.to_string().contains("async"),
        "{err}"
    );

    let source = r"
pub fn bad() {
    let _f = |(a, b)| a + b;
}
";
    let err = parse_module(source, "control.closures").expect_err("tuple param");
    let msg = err.to_string();
    assert!(
        msg.contains("plain identifiers") || msg.contains("unsupported"),
        "{msg}"
    );
}

#[test]
fn lowers_elem_value_name_to_payload() {
    let source = r#"
pub fn f() {
    let _v = ElemValue::Name("iron-plate");
}
"#;
    let module = must_ok_parse(parse_module(source, "control"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::VariableDecl { value, .. } = &function.body.statements[0] else {
        panic!("expected let");
    };
    assert!(matches!(
        value,
        Expression::Literal(Literal::String(s)) if s == "iron-plate"
    ));
}

#[test]
fn lowers_prototype_filter_builder_call() {
    let source = r#"
pub fn f() {
    let _f = EntityPrototypeFilter::name("furnace");
}
"#;
    let module = must_ok_parse(parse_module(source, "control"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::VariableDecl { value, .. } = &function.body.statements[0] else {
        panic!("expected let");
    };
    let Expression::StructLiteral { fields, .. } = value else {
        panic!("expected struct literal, got {value:?}");
    };
    assert_eq!(
        fields,
        &[
            (
                "filter".to_string(),
                Expression::Literal(Literal::String("name".to_string()))
            ),
            (
                "name".to_string(),
                Expression::Literal(Literal::String("furnace".to_string()))
            ),
        ]
    );
}

#[test]
fn lowers_gui_element_child_index() {
    let source = r#"
pub fn f(frame: LuaGuiElement) {
    let _child = frame["title"];
}
"#;
    let module = must_ok_parse(parse_module(source, "control"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::VariableDecl { value, .. } = &function.body.statements[0] else {
        panic!("expected let");
    };
    assert!(
        matches!(value, Expression::Index { .. }),
        "expected Index, got {value:?}"
    );
}

#[test]
fn lowers_clear_method_call() {
    let source = r#"
pub fn f(parent: LuaGuiElement) {
    parent.clear();
}
"#;
    let module = must_ok_parse(parse_module(source, "control.clear"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::Expr(expr) = &function.body.statements[0] else {
        panic!("expected expr stmt, got {:?}", function.body.statements[0]);
    };
    assert!(
        matches!(expr, Expression::MethodCall { method, .. } if method == "clear"),
        "expected MethodCall clear, got {expr:?}"
    );
    let lua = factorio_codegen::LuaGenerator::new()
        .generate_module(&module)
        .expect("lua");
    assert!(
        lua.contains("clear()"),
        "expected clear() call in lua, got:\n{lua}"
    );
}
