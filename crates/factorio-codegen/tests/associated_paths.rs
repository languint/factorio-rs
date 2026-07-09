mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::Function,
    literal::Literal,
    module::{Module, Symbol},
    scope::Scope,
    stage::Stage,
    statement::Statement,
    structure::{Struct, StructField},
    r#type::Type,
};

#[test]
fn rewrites_associated_paths_inside_struct_methods() {
    let module = Module {
        name: "player".to_string(),
        stage: Stage::Control,
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
                    source_type: None,
                }],
                constants: vec![(
                    "DEFAULT_HEALTH".to_string(),
                    Expression::Literal(Literal::Int(100)),
                )],
                methods: vec![Function {
                    name: "new".to_string(),
                    params: vec![],
                    body: Block {
                        statements: vec![Statement::Return(Some(Expression::StructLiteral {
                            fields: vec![(
                                "health".to_string(),
                                Expression::QualifiedPath {
                                    segments: vec![
                                        "MyPlayer".to_string(),
                                        "DEFAULT_HEALTH".to_string(),
                                    ],
                                },
                            )],
                        }))],
                    },
                    doc: None,
                    debug: None,
                    event: None,
                }],
                doc: None,
                debug: None,
            }),
        }],
    };

    let output = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(output.contains(
        "return setmetatable({ health = player.MyPlayer.DEFAULT_HEALTH }, { __index = player.MyPlayer })"
    ));
}
