use factorio_frontend::parse_module;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    module::{Module, Symbol},
    scope::Scope,
    statement::Statement,
    structure::{Struct, StructField},
    r#type::Type,
};

const PLAYER_SOURCE: &str = r"
pub struct MyPlayer {
    health: i64,
}

impl MyPlayer {
    pub fn get_health(&self) -> i64 {
        self.health
    }

    pub fn set_health(&mut self, health: i64) {
        self.health = health;
    }
}
";

#[test]
fn parses_struct_with_methods() {
    let module = parse_module(PLAYER_SOURCE, "player").unwrap();

    assert_eq!(
        module,
        Module {
            name: "player".to_string(),
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
            symbols: vec![Symbol {
                scope: Scope::Public,
                statement: Statement::StructDecl(Struct {
                    name: "MyPlayer".to_string(),
                    fields: vec![StructField {
                        name: "health".to_string(),
                        ty: Type::Int,
                    }],
                    constants: vec![],
                    methods: vec![
                        Function {
                            name: "get_health".to_string(),
                            params: vec![Parameter {
                                name: "self".to_string(),
                                r#type: Type::Void,
                            }],
                            body: Block {
                                statements: vec![Statement::Return(Some(
                                    Expression::FieldAccess {
                                        base: Box::new(Expression::Identifier("self".to_string())),
                                        field: "health".to_string(),
                                    },
                                ))],
                            },
                        },
                        Function {
                            name: "set_health".to_string(),
                            params: vec![
                                Parameter {
                                    name: "self".to_string(),
                                    r#type: Type::Void,
                                },
                                Parameter {
                                    name: "health".to_string(),
                                    r#type: Type::Int,
                                },
                            ],
                            body: Block {
                                statements: vec![Statement::Assignment {
                                    target: Expression::FieldAccess {
                                        base: Box::new(Expression::Identifier("self".to_string())),
                                        field: "health".to_string(),
                                    },
                                    value: Expression::Identifier("health".to_string()),
                                }],
                            },
                        },
                    ],
                }),
            }],
        }
    );
}
