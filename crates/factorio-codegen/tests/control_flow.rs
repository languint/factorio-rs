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
fn generates_while_continue_and_break() {
    let module = Module {
        name: "loops".to_string(),
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
                name: "tick".to_string(),
                params: vec![Parameter {
                    name: "n".to_string(),
                    r#type: Type::Int,
                    source_type: None,
                }],
                body: Block {
                    statements: vec![Statement::While {
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
                            Statement::Continue,
                        ],
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
    assert!(output.contains("while n > 0 do"));
    assert!(output.contains("break"));
    assert!(output.contains("goto __continue_1"));
    assert!(output.contains("::__continue_1::"));
}
