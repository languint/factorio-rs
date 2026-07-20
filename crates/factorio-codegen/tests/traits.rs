mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    literal::Literal,
    module::{Module, Symbol, VTable},
    scope::Scope,
    stage::Stage,
    statement::Statement,
    structure::{Struct, StructField},
    r#type::Type,
};

#[test]
#[allow(clippy::too_many_lines)]
fn generates_vtable_and_fat_pointer() {
    let module = Module {
        name: "traits_demo".to_string(),
        stage: Stage::Control,
        body: Block {
            statements: vec![Statement::StructDecl(Struct {
                name: "Point".to_string(),
                fields: vec![StructField {
                    name: "x".to_string(),
                    ty: Type::Int,
                    source_type: Some("i32".to_string()),
                }],
                constants: vec![],
                methods: vec![Function {
                    name: "show".to_string(),
                    params: vec![Parameter {
                        name: "self".to_string(),
                        r#type: Type::Void,
                        source_type: Some("&self".to_string()),
                    }],
                    body: Block {
                        statements: vec![Statement::Return(Some(Expression::FieldAccess {
                            base: Box::new(Expression::Identifier("self".to_string())),
                            field: "x".to_string(),
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
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![VTable {
            name: "__vt_Display_Point".to_string(),
            concrete_type: "Point".to_string(),
            methods: vec!["show".to_string()],
        }],
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(Function {
                name: "run".to_string(),
                params: vec![],
                body: Block {
                    statements: vec![
                        Statement::VariableDecl {
                            name: "d".to_string(),
                            ty: Type::Void,
                            source_type: None,
                            value: Expression::FatPointer {
                                data: Box::new(Expression::StructLiteral {
                                    struct_name: Some("Point".to_string()),
                                    fields: vec![(
                                        "x".to_string(),
                                        Expression::Literal(Literal::Int(1)),
                                    )],
                                }),
                                vtable: "__vt_Display_Point".to_string(),
                            },
                        },
                        Statement::Return(Some(Expression::DynMethodCall {
                            receiver: Box::new(Expression::Identifier("d".to_string())),
                            method: "show".to_string(),
                            args: vec![],
                        })),
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

    let lua = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(
        lua.contains("local __vt_Display_Point = {"),
        "missing vtable:\n{lua}"
    );
    assert!(
        lua.contains("local Point"),
        "expected forward-declared Point before vtable:\n{lua}"
    );
    let forward_pos = lua.find("local Point").expect("forward");
    let vt_pos = lua.find("local __vt_Display_Point").expect("vtable");
    let assign_pos = lua.find("Point = {}").expect("assign");
    assert!(
        forward_pos < vt_pos && vt_pos < assign_pos,
        "expected forward < vtable < assign in:\n{lua}"
    );
    assert!(
        !lua.contains("local Point = {}"),
        "forward-declared Point must not get a second local:\n{lua}"
    );
    assert!(
        lua.contains("show = function(self, ...) return Point.show(self._data, ...) end,"),
        "missing vtable method:\n{lua}"
    );
    assert!(
        lua.contains("_data =") && lua.contains("_vt = __vt_Display_Point"),
        "missing fat pointer:\n{lua}"
    );
    assert!(lua.contains("d._vt.show(d)"), "missing dyn call:\n{lua}");
}
