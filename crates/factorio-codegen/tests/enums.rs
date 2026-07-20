mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    enumeration::{Enum, EnumVariant, EnumVariantFields},
    expression::Expression,
    function::Function,
    module::Module,
    stage::Stage,
    statement::Statement,
};

#[test]
fn generates_tagged_enum_tables_and_methods() {
    let module = Module {
        name: "messages".to_string(),
        stage: Stage::Control,
        body: Block {
            statements: vec![Statement::EnumDecl(Enum {
                name: "Msg".to_string(),
                variants: vec![
                    EnumVariant {
                        name: "Quit".to_string(),
                        fields: EnumVariantFields::Unit,
                    },
                    EnumVariant {
                        name: "Move".to_string(),
                        fields: EnumVariantFields::Tuple { types: vec![] },
                    },
                ],
                constants: vec![],
                methods: vec![Function {
                    name: "quit".to_string(),
                    params: vec![],
                    body: Block {
                        statements: vec![Statement::Return(Some(Expression::EnumLiteral {
                            enum_name: "Msg".to_string(),
                            variant: "Quit".to_string(),
                            fields: vec![],
                        }))],
                    },
                    doc: None,
                    debug: None,
                    event: None,
                    event_filter: None,
                    export: None,
                }],
                doc: None,
                debug: None,
            })],
        },
        symbols: vec![],
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
    };

    let output = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(output.contains("local Msg = {}"));
    assert!(output.contains("Msg.Quit = { tag = \"Quit\" }"));
    assert!(output.contains("function Msg.quit()"));
    assert!(output.contains("return setmetatable({ tag = \"Quit\" }, { __index = Msg })"));
}
