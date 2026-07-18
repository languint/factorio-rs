mod common;

use common::must_ok;
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

#[test]
fn generates_numeric_ipairs_and_collect_iife() {
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
                name: "collect".to_string(),
                params: vec![],
                body: Block {
                    statements: vec![
                        Statement::ForNumeric {
                            var: "i".to_string(),
                            start: Expression::Literal(Literal::Int(0)),
                            limit: Expression::Identifier("n".to_string()),
                            body: vec![],
                        },
                        Statement::ForIn {
                            var: "value".to_string(),
                            iter: Expression::Identifier("values".to_string()),
                            body: vec![],
                            ipairs: true,
                        },
                        Statement::VariableDecl {
                            name: "out".to_string(),
                            ty: Type::Void,
                            source_type: None,
                            value: collect_iife(),
                        },
                    ],
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
    assert!(output.contains("for i = 0, n do"));
    assert!(output.contains("for _, value in ipairs(values) do"));
    assert!(output.contains("local __out = {}"));
    assert!(output.contains("table.insert(__out, __iter_value)"));
}

fn collect_iife() -> Expression {
    Expression::Call {
        func: Box::new(Expression::Closure {
            params: vec![],
            body: Block {
                statements: vec![
                    Statement::VariableDecl {
                        name: "__out".to_string(),
                        ty: Type::Void,
                        source_type: None,
                        value: Expression::Call {
                            func: Box::new(Expression::QualifiedPath {
                                segments: vec!["Vec".to_string(), "new".to_string()],
                            }),
                            args: vec![],
                        },
                    },
                    Statement::ForNumeric {
                        var: "__iter_item".to_string(),
                        start: Expression::Literal(Literal::Int(0)),
                        limit: Expression::BinaryOp {
                            lhs: Box::new(Expression::Identifier("n".to_string())),
                            op: Operator::Sub,
                            rhs: Box::new(Expression::Literal(Literal::Int(1))),
                        },
                        body: vec![
                            Statement::VariableDecl {
                                name: "__iter_value".to_string(),
                                ty: Type::Void,
                                source_type: None,
                                value: Expression::Identifier("__iter_item".to_string()),
                            },
                            Statement::Expr(Expression::MethodCall {
                                receiver: Box::new(Expression::Identifier("__out".to_string())),
                                method: "push".to_string(),
                                args: vec![Expression::Identifier("__iter_value".to_string())],
                            }),
                        ],
                    },
                    Statement::Return(Some(Expression::Identifier("__out".to_string()))),
                ],
            },
        }),
        args: vec![],
    }
}
