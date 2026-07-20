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
};

#[test]
fn generates_format_concat_for_println() {
    let module = Module {
        name: "on_init".to_string(),
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
                name: "on_init".to_string(),
                params: vec![],
                body: Block {
                    statements: vec![
                        Statement::VariableDecl {
                            name: "health".to_string(),
                            ty: factorio_ir::r#type::Type::Int,
                            source_type: None,
                            value: Expression::Literal(Literal::Int(99)),
                        },
                        Statement::Expr(Expression::Call {
                            func: Box::new(Expression::FieldAccess {
                                base: Box::new(Expression::Identifier("game".to_string())),
                                field: "print".to_string(),
                            }),
                            args: vec![Expression::FormatConcat {
                                parts: vec![
                                    Expression::Literal(Literal::String("health: ".to_string())),
                                    Expression::Identifier("health".to_string()),
                                ],
                            }],
                        }),
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

    assert!(output.contains(r#"game.print("health: " .. health)"#));
}
