use crate::{
    module::Module,
    prune::{
        module_graph::ItemKey,
        reachability::{is_statement_reachable, ModuleReachability},
    },
    statement::Statement,
    structure::Struct,
};

/// Drop unreachable top-level statements and exports from one module.
pub fn prune_module(module: &mut Module, reach: &ModuleReachability) {
    module
        .body
        .statements
        .retain(|statement| is_statement_reachable(statement, reach));
    module
        .symbols
        .retain(|symbol| is_statement_reachable(&symbol.statement, reach));

    for statement in module.body.statements.iter_mut().chain(
        module
            .symbols
            .iter_mut()
            .map(|symbol| &mut symbol.statement),
    ) {
        if let Statement::StructDecl(struct_decl) = statement {
            prune_struct(struct_decl, reach);
        }
    }
}

/// Drop unreachable methods and associated constants from a kept struct declaration.
fn prune_struct(struct_decl: &mut Struct, reach: &ModuleReachability) {
    struct_decl.constants.retain(|(name, _)| {
        reach.items.contains(&ItemKey::StructConstant(
            struct_decl.name.clone(),
            name.clone(),
        ))
    });
    struct_decl.methods.retain(|method| {
        reach.items.contains(&ItemKey::StructMethod(
            struct_decl.name.clone(),
            method.name.clone(),
        ))
    });
}
