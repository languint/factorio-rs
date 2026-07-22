#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::redundant_clone,
    clippy::float_cmp
)]

mod common;

use common::{assert_lua_fragment_parses, must_ok};
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::Function,
    literal::Literal,
    module::{Module, Symbol},
    operator::Operator,
    scope::Scope,
    stage::Stage,
    statement::Statement,
    r#type::Type,
};
use proptest::{prelude::*, test_runner::Config};

const MAX_DEPTH: u32 = 4;

fn arb_ident() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("a".to_string()),
        Just("b".to_string()),
        Just("x".to_string()),
        Just("y".to_string()),
        Just("t".to_string()),
        Just("xs".to_string()),
    ]
}

fn arb_literal() -> impl Strategy<Value = Literal> {
    prop_oneof![
        (0i64..20).prop_map(Literal::Int),
        Just(Literal::Bool(true)),
        Just(Literal::Bool(false)),
        Just(Literal::Nil),
        Just(Literal::String("s".to_string())),
    ]
}

fn arb_operator() -> impl Strategy<Value = Operator> {
    prop_oneof![
        Just(Operator::Add),
        Just(Operator::Sub),
        Just(Operator::Mul),
        Just(Operator::Div),
        Just(Operator::Mod),
        Just(Operator::Eq),
        Just(Operator::Ne),
        Just(Operator::Lt),
        Just(Operator::Le),
        Just(Operator::Gt),
        Just(Operator::Ge),
        Just(Operator::And),
        Just(Operator::Or),
    ]
}

fn arb_expr(depth: u32) -> BoxedStrategy<Expression> {
    if depth == 0 {
        return prop_oneof![
            arb_literal().prop_map(Expression::Literal),
            arb_ident().prop_map(Expression::Identifier),
        ]
        .boxed();
    }

    let leaf = arb_expr(0);
    let nested = arb_expr(depth - 1);

    prop_oneof![
        leaf.clone(),
        (arb_ident(), arb_ident()).prop_map(|(base, field)| Expression::FieldAccess {
            base: Box::new(Expression::Identifier(base)),
            field,
        }),
        (arb_ident(), proptest::collection::vec(nested.clone(), 0..3)).prop_map(|(func, args)| {
            Expression::Call {
                func: Box::new(Expression::Identifier(func)),
                args,
            }
        }),
        (
            arb_ident(),
            prop_oneof![
                Just("clear".to_string()),
                Just("get_health".to_string()),
                Just("len".to_string()),
                Just("push".to_string()),
                Just("caption".to_string()),
            ],
            proptest::collection::vec(nested.clone(), 0..2)
        )
            .prop_map(|(receiver, method, args)| Expression::MethodCall {
                receiver: Box::new(Expression::Identifier(receiver)),
                method,
                args,
            }),
        (nested.clone(), arb_operator(), nested.clone()).prop_map(|(lhs, op, rhs)| {
            Expression::BinaryOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            }
        }),
        nested
            .clone()
            .prop_map(|inner| Expression::Not(Box::new(inner))),
        arb_ident().prop_map(|name| Expression::Len(Box::new(Expression::Identifier(name)))),
        proptest::collection::vec(nested.clone(), 0..3)
            .prop_map(|elements| Expression::Array { elements }),
        (arb_ident(), nested.clone()).prop_map(|(base, key)| Expression::Index {
            base: Box::new(Expression::Identifier(base)),
            key: Box::new(key),
        }),
        (nested.clone(), nested.clone(), nested.clone()).prop_map(|(c, t, e)| Expression::If {
            condition: Box::new(c),
            then_expr: Box::new(t),
            else_expr: Box::new(e),
        }),
        arb_ident().prop_map(|data| Expression::FatPointer {
            data: Box::new(Expression::Identifier(data)),
            vtable: "__vt_T_C".to_string(),
        }),
    ]
    .boxed()
}

fn arb_arith_expr(depth: u32) -> BoxedStrategy<Expression> {
    if depth == 0 {
        return (1i64..10)
            .prop_map(|n| Expression::Literal(Literal::Int(n)))
            .boxed();
    }
    let nested = arb_arith_expr(depth - 1);
    let op = prop_oneof![
        Just(Operator::Add),
        Just(Operator::Sub),
        Just(Operator::Mul),
    ];
    prop_oneof![
        (1i64..10).prop_map(|n| Expression::Literal(Literal::Int(n))),
        (nested.clone(), op, nested).prop_map(|(lhs, op, rhs)| Expression::BinaryOp {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        }),
    ]
    .boxed()
}

fn eval_rust(expr: &Expression) -> Option<i64> {
    match expr {
        Expression::Literal(Literal::Int(n)) => Some(*n),
        Expression::BinaryOp { lhs, op, rhs } => {
            let l = eval_rust(lhs)?;
            let r = eval_rust(rhs)?;
            Some(match op {
                Operator::Add => l + r,
                Operator::Sub => l - r,
                Operator::Mul => l * r,
                _ => return None,
            })
        }
        _ => None,
    }
}

fn eval_lua(expr_lua: &str) -> mlua::Result<f64> {
    let lua = mlua::Lua::new();
    lua.load(format!("return ({expr_lua})")).eval()
}

proptest! {
    #![proptest_config(Config::with_cases(128))]

    #[test]
    fn generated_expressions_parse(expr in arb_expr(MAX_DEPTH)) {
        let lua = LuaGenerator::new().generate_expression(&expr);
        assert_lua_fragment_parses(&lua);
    }

    #[test]
    fn generated_modules_parse(
        exprs in proptest::collection::vec(arb_expr(2), 1..4)
    ) {
        let statements: Vec<Statement> = exprs
            .into_iter()
            .enumerate()
            .map(|(i, value)| Statement::VariableDecl {
                name: format!("v{i}"),
                ty: Type::Void,
                source_type: None,
                value,
            })
            .collect();

        let module = Module {
            name: "fuzz".to_string(),
            stage: Stage::Control,
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
            locales: vec![],
            pending_locales: vec![],
            vtables: vec![],
            symbols: vec![Symbol {
                scope: Scope::Public,
                statement: Statement::FunctionDecl(Function {
                    name: "run".to_string(),
                    params: vec![],
                    body: Block { statements },
                    doc: None,
                    debug: None,
                    event: None,
                    event_filter: None,
                    export: None,
                    inline: false,
                }),
            }],
        };

        let _ = must_ok(LuaGenerator::new().generate_module(&module));
    }

    #[test]
    fn arithmetic_matches_mlua(expr in arb_arith_expr(3)) {
        let expected = eval_rust(&expr).expect("arith strategy only yields ints");
        let lua = LuaGenerator::new().generate_expression(&expr);
        assert_lua_fragment_parses(&lua);
        let got = eval_lua(&lua).map_err(|e| TestCaseError::fail(e.to_string()))?;
        #[allow(clippy::as_conversions, clippy::cast_precision_loss)]
        let expected_f = expected as f64;
        prop_assert_eq!(got, expected_f, "lua `{}`", lua);
    }
}
