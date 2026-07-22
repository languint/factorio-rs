#[macro_use]
mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    literal::Literal,
    module::{Module, Symbol},
    scope::Scope,
    stage::Stage,
    statement::Statement,
    r#type::Type,
};

#[test]
fn generates_module_with_private_helper_and_exported_handler() {
    let module = Module {
        name: "bound_detector".to_string(),
        stage: Stage::Control,
        body: Block {
            statements: vec![Statement::FunctionDecl(Function {
                name: "helper".to_string(),
                params: vec![],
                body: Block {
                    statements: vec![Statement::Return(Some(Expression::Literal(Literal::Int(
                        1,
                    ))))],
                },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
                inline: false,
            })],
        },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(Function {
                name: "on_init".to_string(),
                params: vec![Parameter {
                    name: "event".to_string(),
                    r#type: Type::Void,
                    source_type: None,
                }],
                body: Block {
                    statements: vec![Statement::VariableDecl {
                        name: "count".to_string(),
                        ty: Type::Int,
                        source_type: None,
                        value: Expression::Literal(Literal::Int(0)),
                    }],
                },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
                inline: false,
            }),
        }],
    };

    assert_module_snapshot!(
        "module_with_private_helper_and_exported_handler",
        LuaGenerator::new().generate_module(&module)
    );
}

#[test]
fn omits_unreachable_private_helper_when_pruned() {
    use factorio_ir::prune::prune_modules;

    let mut module = Module {
        name: "bound_detector".to_string(),
        stage: Stage::Control,
        body: Block {
            statements: vec![Statement::FunctionDecl(Function {
                name: "helper".to_string(),
                params: vec![],
                body: Block {
                    statements: vec![Statement::Return(Some(Expression::Literal(Literal::Int(
                        1,
                    ))))],
                },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
                inline: false,
            })],
        },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(Function {
                name: "on_init".to_string(),
                params: vec![Parameter {
                    name: "event".to_string(),
                    r#type: Type::Void,
                    source_type: None,
                }],
                body: Block {
                    statements: vec![Statement::VariableDecl {
                        name: "count".to_string(),
                        ty: Type::Int,
                        source_type: None,
                        value: Expression::Literal(Literal::Int(0)),
                    }],
                },
                doc: None,
                debug: None,
                event: Some("on_init".to_string()),
                event_filter: None,
                export: None,
                inline: false,
            }),
        }],
    };

    prune_modules(std::slice::from_mut(&mut module));

    assert_module_snapshot!(
        "omits_unreachable_private_helper_when_pruned",
        LuaGenerator::new().generate_module(&module)
    );
}

#[test]
fn forward_declares_private_functions_for_later_callers() {
    let module = Module {
        name: "control".to_string(),
        stage: Stage::Control,
        body: Block {
            statements: vec![
                Statement::FunctionDecl(Function {
                    name: "caller".to_string(),
                    params: vec![],
                    body: Block {
                        statements: vec![Statement::Return(Some(Expression::Call {
                            func: Box::new(Expression::Identifier("helper".to_string())),
                            args: vec![],
                        }))],
                    },
                    doc: None,
                    debug: None,
                    event: None,
                    event_filter: None,
                    export: None,
                    inline: false,
                }),
                Statement::FunctionDecl(Function {
                    name: "helper".to_string(),
                    params: vec![],
                    body: Block {
                        statements: vec![Statement::Return(Some(Expression::Literal(
                            Literal::Int(1),
                        )))],
                    },
                    doc: None,
                    debug: None,
                    event: None,
                    event_filter: None,
                    export: None,
                    inline: false,
                }),
            ],
        },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
        symbols: vec![],
    };

    let output = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        output.contains("local caller, helper\n"),
        "private fns must be forward-declared:\n{output}"
    );
    assert!(
        output.contains("function caller()\n") && output.contains("return helper()"),
        "caller should invoke the local helper upvalue:\n{output}"
    );
    assert!(
        !output.contains("local function "),
        "forward-declared fns must not use `local function`:\n{output}"
    );
}

#[test]
fn qualifies_exported_const_identifiers() {
    let module = Module {
        name: "gui".to_string(),
        stage: Stage::Control,
        body: Block { statements: vec![] },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
        symbols: vec![
            Symbol {
                scope: Scope::Public,
                statement: Statement::VariableDecl {
                    name: "ROOT".to_string(),
                    ty: Type::Void,
                    source_type: None,
                    value: Expression::Literal(Literal::String("milestones_gui".to_string())),
                },
            },
            Symbol {
                scope: Scope::Public,
                statement: Statement::FunctionDecl(Function {
                    name: "open".to_string(),
                    params: vec![],
                    body: Block {
                        statements: vec![Statement::Expr(Expression::Call {
                            func: Box::new(Expression::QualifiedPath {
                                segments: vec!["runtime".to_string(), "mount".to_string()],
                            }),
                            args: vec![Expression::Identifier("ROOT".to_string())],
                        })],
                    },
                    doc: None,
                    debug: None,
                    event: None,
                    event_filter: None,
                    export: None,
                    inline: false,
                }),
            },
        ],
    };

    let output = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        output.contains("gui.ROOT = \"milestones_gui\""),
        "pub const must be on the module table:\n{output}"
    );
    assert!(
        output.contains("runtime.mount(gui.ROOT)"),
        "same-module uses of pub const must qualify:\n{output}"
    );
    assert!(
        !output.contains("runtime.mount(ROOT)"),
        "bare pub const name would be nil at runtime:\n{output}"
    );
}

#[test]
fn qualifies_exported_function_identifiers_used_as_values() {
    let module = Module {
        name: "control".to_string(),
        stage: Stage::Control,
        body: Block { statements: vec![] },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
        symbols: vec![
            Symbol {
                scope: Scope::Public,
                statement: Statement::FunctionDecl(Function {
                    name: "greet".to_string(),
                    params: vec![],
                    body: Block { statements: vec![] },
                    doc: None,
                    debug: None,
                    event: None,
                    event_filter: None,
                    export: None,
                    inline: false,
                }),
            },
            Symbol {
                scope: Scope::Public,
                statement: Statement::FunctionDecl(Function {
                    name: "on_init".to_string(),
                    params: vec![],
                    body: Block {
                        statements: vec![Statement::Expr(Expression::MethodCall {
                            receiver: Box::new(Expression::Identifier("commands".to_string())),
                            method: "add_command".to_string(),
                            args: vec![
                                Expression::Literal(Literal::String("greet".to_string())),
                                Expression::Identifier("greet".to_string()),
                            ],
                        })],
                    },
                    doc: None,
                    debug: None,
                    event: Some("on_init".to_string()),
                    event_filter: None,
                    export: None,
                    inline: false,
                }),
            },
        ],
    };

    let output = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        output.contains("commands.add_command(\"greet\", control.greet)"),
        "pub fn references must qualify through the module table, got:\n{output}"
    );
    assert!(
        !output.contains("add_command(\"greet\", greet)"),
        "bare exported fn name would be nil at runtime:\n{output}"
    );
}
