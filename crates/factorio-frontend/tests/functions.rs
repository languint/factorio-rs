use factorio_frontend::parse_module;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    module::{Module, Symbol},
    scope::Scope,
    statement::Statement,
    r#type::Type,
};

#[test]
fn parses_method_with_self() {
    let source = r"
pub fn reset(&mut self, player: ()) {
    return;
}
";

    let module = parse_module(source, "player_util").unwrap();

    assert_eq!(
        module,
        Module {
            name: "player_util".to_string(),
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
            symbols: vec![Symbol {
                scope: Scope::Public,
                statement: Statement::FunctionDecl(Function {
                    name: "reset".to_string(),
                    params: vec![
                        Parameter {
                            name: "self".to_string(),
                            r#type: Type::Void,
                        },
                        Parameter {
                            name: "player".to_string(),
                            r#type: Type::Void,
                        },
                    ],
                    body: Block {
                        statements: vec![Statement::Return(None)],
                    },
                }),
            }],
        }
    );
}

#[test]
fn parses_implicit_return() {
    let source = r"
fn helper() -> i64 {
    1
}
";

    let module = parse_module(source, "example").unwrap();

    assert_eq!(
        module.body.statements,
        vec![Statement::FunctionDecl(Function {
            name: "helper".to_string(),
            params: vec![],
            body: Block {
                statements: vec![Statement::Return(Some(Expression::Literal(
                    factorio_ir::literal::Literal::Int(1),
                )))],
            },
        })]
    );
}
