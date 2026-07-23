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
fn lowers_ok_unit() {
    let source = r#"
pub enum GreetError {
    EmptyName,
}

type GreetResult<T> = Result<T, GreetError>;

pub fn greet() -> GreetResult<()> {
    Ok(())
}
"#;
    let module = must_ok_parse(parse_module(source, "control.greet_unit"));
    let greet = module
        .symbols
        .iter()
        .find_map(|s| match &s.statement {
            Statement::FunctionDecl(f) if f.name == "greet" => Some(f),
            _ => None,
        })
        .expect("greet");
    let Statement::Return(Some(Expression::StructLiteral {
        fields,
        struct_name,
    })) = &greet.body.statements[0]
    else {
        panic!("expected Ok(()), got {:?}", greet.body.statements);
    };
    assert_eq!(struct_name.as_deref(), Some("Result"));
    assert_eq!(
        fields,
        &vec![("ok".to_string(), Expression::Literal(Literal::Nil))]
    );
}

#[test]
fn lowers_ok_err_constructors() {
    let source = r#"
pub fn ok_one() -> Result<i32, String> {
    Ok(1)
}

pub fn err_msg() -> Result<i32, String> {
    Err("no")
}
"#;
    let module = must_ok_parse(parse_module(source, "control.result_ctors"));
    let ok_fn = module
        .symbols
        .iter()
        .find_map(|s| match &s.statement {
            Statement::FunctionDecl(f) if f.name == "ok_one" => Some(f),
            _ => None,
        })
        .expect("ok_one");
    let Statement::Return(Some(Expression::StructLiteral {
        fields,
        struct_name,
    })) = &ok_fn.body.statements[0]
    else {
        panic!("expected Ok struct, got {:?}", ok_fn.body.statements);
    };
    assert_eq!(struct_name.as_deref(), Some("Result"));
    assert_eq!(
        fields,
        &vec![("ok".to_string(), Expression::Literal(Literal::Int(1)))]
    );

    let err_fn = module
        .symbols
        .iter()
        .find_map(|s| match &s.statement {
            Statement::FunctionDecl(f) if f.name == "err_msg" => Some(f),
            _ => None,
        })
        .expect("err_msg");
    let Statement::Return(Some(Expression::StructLiteral { fields, .. })) =
        &err_fn.body.statements[0]
    else {
        panic!("expected Err struct");
    };
    assert_eq!(fields[0].0, "err");
    assert_eq!(
        fields[0].1,
        Expression::Literal(Literal::String("no".to_string()))
    );
}

#[test]
fn lowers_try_operator() {
    let source = r#"
pub fn parse(_: &str) -> Result<i32, String> {
    Ok(1)
}

pub fn load(name: &str) -> Result<i32, String> {
    let n = parse(name)?;
    Ok(n + 1)
}
"#;
    let module = must_ok_parse(parse_module(source, "control.result_try"));
    let function = module
        .symbols
        .iter()
        .find_map(|s| match &s.statement {
            Statement::FunctionDecl(f) if f.name == "load" => Some(f),
            _ => None,
        })
        .expect("load");

    // local __try_N = parse(name)
    // if __try_N.err ~= nil then return __try_N end
    // local n = __try_N.ok
    // return { ok = n + 1 }
    assert!(function.body.statements.len() >= 4);
    let Statement::VariableDecl { name: try_name, .. } = &function.body.statements[0] else {
        panic!("expected try temp, got {:?}", function.body.statements[0]);
    };
    assert!(try_name.starts_with("__try_"));
    let Statement::Conditional {
        condition,
        then_block,
        ..
    } = &function.body.statements[1]
    else {
        panic!("expected err check");
    };
    assert_eq!(
        condition,
        &Expression::BinaryOp {
            lhs: Box::new(Expression::FieldAccess {
                base: Box::new(Expression::Identifier(try_name.clone())),
                field: "err".to_string(),
            }),
            op: Operator::Ne,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }
    );
    assert!(matches!(
        then_block.as_slice(),
        [Statement::Return(Some(Expression::Identifier(n)))] if n == try_name
    ));
    let Statement::VariableDecl {
        name: n_name,
        value,
        ..
    } = &function.body.statements[2]
    else {
        panic!("expected n binding");
    };
    assert_eq!(n_name, "n");
    assert_eq!(
        value,
        &Expression::FieldAccess {
            base: Box::new(Expression::Identifier(try_name.clone())),
            field: "ok".to_string(),
        }
    );
}

#[test]
fn lowers_if_let_ok() {
    let source = r#"
pub fn handle(r: Result<i32, String>) {
    if let Ok(n) = r {
        return n;
    }
}
"#;
    let module = must_ok_parse(parse_module(source, "control.result_if_let"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    assert!(matches!(
        function.body.statements.as_slice(),
        [
            Statement::VariableDecl { .. },
            Statement::Conditional {
                condition: Expression::BinaryOp {
                    op: Operator::Eq,
                    ..
                },
                then_block,
                ..
            }
        ] if matches!(
            then_block.as_slice(),
            [
                Statement::VariableDecl { name, .. },
                Statement::Return(Some(Expression::Identifier(ret)))
            ] if name == "n" && ret == "n"
        )
    ));
}

#[test]
fn lowers_match_ok_err() {
    let source = r#"
pub fn unwrap_or_zero(r: Result<i32, String>) -> i32 {
    match r {
        Ok(n) => n,
        Err(_) => 0,
    }
}
"#;
    let module = must_ok_parse(parse_module(source, "control.result_match"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let body = &function.body.statements;
    let Statement::Conditional {
        condition,
        then_block,
        ..
    } = &body[1]
    else {
        panic!("expected Ok arm, got {body:?}");
    };
    assert!(matches!(
        condition,
        Expression::BinaryOp {
            op: Operator::Eq,
            lhs,
            ..
        } if matches!(
            lhs.as_ref(),
            Expression::FieldAccess { field, .. } if field == "err"
        )
    ));
    assert!(matches!(
        then_block.as_slice(),
        [
            Statement::VariableDecl { name, .. },
            Statement::Return(Some(Expression::Identifier(ret)))
        ] if name == "n" && ret == "n"
    ));
}

#[test]
fn lowers_result_is_ok_map() {
    let source = r#"
pub fn bump(r: Result<i32, String>) -> Result<i32, String> {
    if r.is_ok() {
        return r.map(|n| n + 1);
    }
    r
}
"#;
    let module = must_ok_parse(parse_module(source, "control.result_methods"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::Conditional { condition, .. } = &function.body.statements[0] else {
        panic!("expected if");
    };
    assert!(matches!(
        condition,
        Expression::BinaryOp {
            op: Operator::Eq,
            lhs,
            ..
        } if matches!(
            lhs.as_ref(),
            Expression::FieldAccess { field, .. } if field == "err"
        )
    ));
}

#[test]
fn lowers_option_ok_or() {
    let source = r#"
pub fn place(entity: Option<i32>) -> Result<i32, String> {
    entity.ok_or("missing")
}
"#;
    let module = must_ok_parse(parse_module(source, "control.option_ok_or"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    // Hoisted to statement if + result temp (no IIFE).
    let body = &function.body.statements;
    assert!(
        matches!(
            body.last(),
            Some(Statement::Return(Some(Expression::Identifier(_))))
        ),
        "expected return of result temp, got {body:?}"
    );
    let Some(Statement::Conditional {
        then_block,
        else_block,
        ..
    }) = body
        .iter()
        .find(|s| matches!(s, Statement::Conditional { .. }))
    else {
        panic!("expected hoisted conditional, got {body:?}");
    };
    assert!(matches!(
        then_block.as_slice(),
        [Statement::Assignment {
            value: Expression::StructLiteral { fields, .. },
            ..
        }] if fields[0].0 == "ok"
    ));
    assert!(matches!(
        else_block.as_slice(),
        [Statement::Assignment {
            value: Expression::StructLiteral { fields, .. },
            ..
        }] if fields[0].0 == "err"
    ));
}

#[test]
fn ok_or_question_skips_ok_result_table() {
    let source = r#"
pub fn place(entity: Option<i32>) -> Result<i32, String> {
    let v = entity.ok_or("missing")?;
    Ok(v)
}
"#;
    let module = must_ok_parse(parse_module(source, "control.ok_or_try"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let body = &function.body.statements;
    let err_early_return = body.iter().any(|s| {
        matches!(
            s,
            Statement::Conditional { then_block, .. }
                if then_block.iter().any(|t| {
                    matches!(
                        t,
                        Statement::Return(Some(Expression::StructLiteral { fields, .. }))
                            if fields.len() == 1 && fields[0].0 == "err"
                    )
                })
        )
    });
    assert!(
        err_early_return,
        "expected nil -> return {{ err = ... }}, got {body:?}"
    );

    let loads_ok_field = format!("{body:?}").contains("field: \"ok\"");
    assert!(
        !loads_ok_field,
        "ok_or? should not read `.ok` from a Result table: {body:?}"
    );
}

#[test]
fn ok_or_binds_side_effecting_receiver_once() {
    use factorio_codegen::LuaGenerator;

    let source = r#"
pub fn try_place(surface: LuaSurface, params: i32) -> Result<i32, String> {
    surface.create_entity(params).ok_or("failed")
}
"#;
    let module = must_ok_parse(parse_module(source, "control.ok_or_once"));
    let lua = LuaGenerator::new()
        .generate_module(&module)
        .expect("generate");
    let count = lua.matches("create_entity").count();
    assert_eq!(
        count, 1,
        "create_entity should be evaluated once, lua was:\n{lua}"
    );
    assert!(
        lua.contains("local __o_"),
        "expected bound temp `__o_N`, lua was:\n{lua}"
    );
}
