use factorio_frontend::parse_module;
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

    let module = parse_module(source, "math_util").unwrap();

    assert_eq!(
        module,
        Module {
            name: "math_util".to_string(),
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
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
        }
    );
}

#[test]
fn parses_assignment() {
    let source = r"
pub fn bump(counter: i32) {
    counter = counter + 1;
}
";

    let module = parse_module(source, "counter").unwrap();
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function declaration");
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
