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

use self::{apply::prune_module, module_graph::ModuleGraph, reachability::compute_reachability};

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
                })],
            },
            imports: vec![],
            submodules: vec![],
            locales: vec![],
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
    fn prunes_unused_public_exports() {
        let mut modules = vec![Module {
            name: "control".to_string(),
            stage: Stage::Control,
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
            locales: vec![],
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
                    }),
                },
            ],
        }];

        prune_modules(&mut modules);

        assert_eq!(modules[0].symbols.len(), 1);
    }
}
