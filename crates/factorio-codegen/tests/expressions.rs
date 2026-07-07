use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    literal::Literal,
    module::{Module, Symbol},
    operator::Operator,
    scope::Scope,
    statement::Statement,
    r#type::Type,
};

#[test]
fn generates_binary_ops_and_conditionals() {
    let module = Module {
        name: "math_util".to_string(),
        body: Block { statements: vec![] },
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(Function {
                name: "add".to_string(),
                params: vec![
                    Parameter {
                        name: "a".to_string(),
                        r#type: Type::Int,
                    },
                    Parameter {
                        name: "b".to_string(),
                        r#type: Type::Int,
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
            }),
        }],
    };

    let output = LuaGenerator::new().generate_module(&module).unwrap();

    assert!(output.contains("if (a == 0) then"));
    assert!(output.contains("return (a + b)"));
}
