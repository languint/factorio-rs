mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    literal::Literal,
    module::{Module, Symbol},
    operator::Operator,
    scope::Scope,
    stage::Stage,
    statement::Statement,
    r#type::Type,
};

#[test]
fn generates_binary_ops_and_conditionals() {
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
                        source_type: None,
                    },
                    Parameter {
                        name: "b".to_string(),
                        r#type: Type::Int,
                        source_type: None,
                    },
                ],
                body: Block {
                    statements: vec![Statement::Conditional {
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
                    }],
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

    assert!(output.contains("if a == 0 then"));
    assert!(output.contains("return a + b"));
}
