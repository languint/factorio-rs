//! Dead-code elimination for lowered IR modules.
//!
//! Pruning runs after lowering and before Lua codegen. Reachability starts from
//! event handlers registered with `#[factorio_rs::event]`, then follows call graphs
//! within and across modules. Anything not reached is removed from the IR so it
//! is never emitted to Lua.

mod apply;
mod items;
mod module_graph;
mod reachability;
mod references;
mod struct_utils;

use crate::module::Module;

use self::{apply::prune_module, reachability::compute_reachability};

pub use module_graph::ModuleGraph;
pub use struct_utils::{
    find_struct_constant, find_struct_constant_in_module_tree, struct_owner_module,
};

/// Remove unreachable functions and exports from transpiled modules.
///
/// When dead-code pruning is enabled for the active transpile profile, the build
/// collects all lowered modules, runs this pass, then generates Lua from the pruned IR.
pub fn prune_modules(modules: &mut [Module]) {
    if modules.is_empty() {
        return;
    }

    let graph = ModuleGraph::new(modules);
    let reachability = compute_reachability(&graph);

    for module in modules {
        if let Some(reach) = reachability.get(&module.name) {
            prune_module(module, reach);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        block::Block,
        function::Function,
        module::{Module, Symbol},
        scope::Scope,
        stage::Stage,
        statement::Statement,
    };

    use super::prune_modules;

    #[test]
    fn prunes_unreachable_private_functions() {
        let mut modules = vec![Module {
            name: "control".to_string(),
            stage: Stage::Control,
            body: Block {
                statements: vec![Statement::FunctionDecl(Function {
                    name: "add".to_string(),
                    params: vec![],
                    body: Block { statements: vec![] },
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
                    params: vec![],
                    body: Block { statements: vec![] },
                    doc: None,
                    debug: None,
                    event: Some("on_init".to_string()),
                    event_filter: None,
                    export: None,
                    inline: false,
                }),
            }],
        }];

        prune_modules(&mut modules);

        assert!(modules[0].body.statements.is_empty());
        assert_eq!(modules[0].symbols.len(), 1);
        assert_eq!(
            match &modules[0].symbols[0].statement {
                Statement::FunctionDecl(function) => function.name.as_str(),
                _ => panic!("expected function"),
            },
            "on_init"
        );
    }

    #[test]
    fn keeps_functions_passed_as_values() {
        use crate::expression::Expression;

        let mut modules = vec![Module {
            name: "control".to_string(),
            stage: Stage::Control,
            body: Block {
                statements: vec![Statement::FunctionDecl(Function {
                    name: "greet".to_string(),
                    params: vec![],
                    body: Block { statements: vec![] },
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
                    params: vec![],
                    body: Block {
                        statements: vec![Statement::Expr(Expression::MethodCall {
                            receiver: Box::new(Expression::Identifier("commands".to_string())),
                            method: "add_command".to_string(),
                            args: vec![
                                Expression::Literal(crate::literal::Literal::String(
                                    "greet".to_string(),
                                )),
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
            }],
        }];

        prune_modules(&mut modules);

        assert_eq!(modules[0].body.statements.len(), 1);
        assert_eq!(
            match &modules[0].body.statements[0] {
                Statement::FunctionDecl(function) => function.name.as_str(),
                _ => panic!("expected greet to remain"),
            },
            "greet"
        );
    }

    #[test]
    fn prunes_unused_public_exports() {
        let mut modules = vec![Module {
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
                        name: "unused".to_string(),
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
                        body: Block { statements: vec![] },
                        doc: None,
                        debug: None,
                        event: Some("on_init".to_string()),
                        event_filter: None,
                        export: None,
                        inline: false,
                    }),
                },
            ],
        }];

        prune_modules(&mut modules);

        assert_eq!(modules[0].symbols.len(), 1);
    }

    #[test]
    fn keeps_public_shared_structs_as_library_api() {
        use crate::structure::{Struct, StructField};
        use crate::r#type::Type;

        let mut modules = vec![Module {
            name: "shared.frame".to_string(),
            stage: Stage::Shared,
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
            locales: vec![],
            pending_locales: vec![],
            vtables: vec![],
            symbols: vec![Symbol {
                scope: Scope::Public,
                statement: Statement::StructDecl(Struct {
                    name: "Frame".to_string(),
                    fields: vec![StructField {
                        name: "caption".to_string(),
                        ty: Type::Str,
                        source_type: None,
                    }],
                    constants: vec![],
                    methods: vec![Function {
                        name: "new".to_string(),
                        params: vec![],
                        body: Block { statements: vec![] },
                        doc: None,
                        debug: None,
                        event: None,
                        event_filter: None,
                        export: None,
                        inline: false,
                    }],
                    doc: None,
                    debug: None,
                }),
            }],
        }];

        prune_modules(&mut modules);

        assert_eq!(modules[0].symbols.len(), 1);
        let Statement::StructDecl(frame) = &modules[0].symbols[0].statement else {
            panic!("expected Frame struct to remain");
        };
        assert_eq!(frame.name, "Frame");
        assert_eq!(frame.methods.len(), 1);
        assert_eq!(frame.methods[0].name, "new");
    }
}
