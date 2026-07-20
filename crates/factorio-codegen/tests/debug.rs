mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    debug::FunctionDebug,
    expression::Expression,
    function::{Function, Parameter},
    module::{Module, Symbol},
    scope::Scope,
    stage::Stage,
    statement::Statement,
    r#type::Type,
};

#[test]
fn debug_level_one_adds_type_comments_to_functions() {
    let module = Module {
        name: "math_util".to_string(),
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
                name: "add".to_string(),
                params: vec![
                    Parameter {
                        name: "a".to_string(),
                        r#type: Type::Int,
                        source_type: Some("i64".to_string()),
                    },
                    Parameter {
                        name: "b".to_string(),
                        r#type: Type::Int,
                        source_type: Some("i64".to_string()),
                    },
                ],
                body: Block {
                    statements: vec![Statement::Return(Some(Expression::BinaryOp {
                        lhs: Box::new(Expression::Identifier("a".to_string())),
                        op: factorio_ir::operator::Operator::Add,
                        rhs: Box::new(Expression::Identifier("b".to_string())),
                    }))],
                },
                doc: None,
                debug: Some(FunctionDebug {
                    header_comment: "pub fn add(a: i64, b: i64) -> i64".to_string(),
                    return_type: Some("i64".to_string()),
                }),
                event: None,
                event_filter: None,
                export: None,
            }),
        }],
    };

    let output = must_ok(LuaGenerator::with_debug_level(1).generate_module(&module));

    assert!(output.contains("-- pub fn add(a: i64, b: i64) -> i64"));
    assert!(output.contains("function mathUtil.add(a --[[ i64 ]], b --[[ i64 ]]) --[[ -> i64 ]]"));
}

#[test]
fn debug_level_zero_adds_header_without_inline_types() {
    let module = Module {
        name: "math_util".to_string(),
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
                name: "add".to_string(),
                params: vec![
                    Parameter {
                        name: "a".to_string(),
                        r#type: Type::Int,
                        source_type: Some("i64".to_string()),
                    },
                    Parameter {
                        name: "b".to_string(),
                        r#type: Type::Int,
                        source_type: Some("i64".to_string()),
                    },
                ],
                body: Block { statements: vec![] },
                doc: None,
                debug: Some(FunctionDebug {
                    header_comment: "pub fn add(a: i64, b: i64) -> i64".to_string(),
                    return_type: Some("i64".to_string()),
                }),
                event: None,
                event_filter: None,
                export: None,
            }),
        }],
    };

    let output = must_ok(LuaGenerator::with_debug_level(0).generate_module(&module));

    assert!(output.contains("-- pub fn add(a: i64, b: i64) -> i64"));
    assert!(output.contains("function mathUtil.add(a, b)"));
    assert!(!output.contains("--[[ i64 ]]"));
}
