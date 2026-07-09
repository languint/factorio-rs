use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    module::Module,
    prune::{
        items::find_function,
        module_graph::{ItemKey, ModuleGraph},
        references::{collect_references_from_expression, collect_references_from_function},
        struct_utils,
    },
    statement::Statement,
    structure::Struct,
};

/// The set of IR items in one module that must be kept in generated Lua.
#[derive(Debug, Default)]
pub struct ModuleReachability {
    pub items: HashSet<ItemKey>,
}

/// Walk the call graph from event-handler roots and collect reachable items per module.
pub fn compute_reachability(graph: &ModuleGraph<'_>) -> HashMap<String, ModuleReachability> {
    let mut reachability = graph
        .modules
        .iter()
        .map(|module| (module.name.clone(), ModuleReachability::default()))
        .collect::<HashMap<_, _>>();

    let mut pending = VecDeque::new();

    for module in graph.modules {
        for symbol in &module.symbols {
            if let Statement::FunctionDecl(function) = &symbol.statement
                && function.event.is_some()
            {
                enqueue_item(
                    &mut reachability,
                    &mut pending,
                    &module.name,
                    ItemKey::Function(function.name.clone()),
                );
            }
        }
    }

    while let Some((module_name, item)) = pending.pop_front() {
        expand_reachable_item(graph, &module_name, &item, &mut reachability, &mut pending);
    }

    reachability
}

/// Expand one reachable item by enqueueing everything it references.
fn expand_reachable_item(
    graph: &ModuleGraph<'_>,
    module_name: &str,
    item: &ItemKey,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    let Some(module) = graph.get(module_name) else {
        return;
    };

    match item {
        ItemKey::Function(name) => expand_reachable_function(
            graph, module, name, reachability, pending,
        ),
        ItemKey::Struct(name) => {
            expand_reachable_struct(graph, module, module_name, name, reachability, pending);
        }
        ItemKey::StructMethod(struct_name, method_name) => expand_reachable_struct_method(
            graph,
            module,
            module_name,
            struct_name,
            method_name,
            reachability,
            pending,
        ),
        ItemKey::StructConstant(struct_name, constant_name) => expand_reachable_struct_constant(
            graph,
            module,
            module_name,
            struct_name,
            constant_name,
            reachability,
            pending,
        ),
    }
}

fn expand_reachable_function(
    graph: &ModuleGraph<'_>,
    module: &Module,
    name: &str,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    if let Some(function) = find_function(module, name) {
        collect_references_from_function(graph, module, function, reachability, pending);
    }
}

fn expand_reachable_struct(
    graph: &ModuleGraph<'_>,
    module: &Module,
    module_name: &str,
    name: &str,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    let Some(struct_decl) = struct_utils::find_struct(module, name) else {
        return;
    };

    for (constant, value) in &struct_decl.constants {
        enqueue_item(
            reachability,
            pending,
            module_name,
            ItemKey::StructConstant(name.to_string(), constant.clone()),
        );
        collect_references_from_expression(
            graph,
            module,
            value,
            &HashMap::new(),
            reachability,
            pending,
        );
    }
    for method in &struct_decl.methods {
        enqueue_item(
            reachability,
            pending,
            module_name,
            ItemKey::StructMethod(name.to_string(), method.name.clone()),
        );
    }
}

fn expand_reachable_struct_method(
    graph: &ModuleGraph<'_>,
    module: &Module,
    module_name: &str,
    struct_name: &str,
    method_name: &str,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    if let Some(method) = struct_utils::find_struct_method(module, struct_name, method_name) {
        collect_references_from_function(graph, module, method, reachability, pending);
        return;
    }

    let Some((owner_module, method)) = struct_utils::find_struct_method_in_module_tree(
        graph,
        module_name,
        struct_name,
        method_name,
    ) else {
        return;
    };

    enqueue_item(
        reachability,
        pending,
        &owner_module,
        ItemKey::StructMethod(struct_name.to_string(), method_name.to_string()),
    );
    if let Some(owner) = graph.get(&owner_module) {
        collect_references_from_function(graph, owner, method, reachability, pending);
    }
}

fn expand_reachable_struct_constant(
    graph: &ModuleGraph<'_>,
    module: &Module,
    module_name: &str,
    struct_name: &str,
    constant_name: &str,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    if let Some(value) = struct_utils::find_struct_constant(module, struct_name, constant_name) {
        collect_references_from_expression(
            graph,
            module,
            value,
            &HashMap::new(),
            reachability,
            pending,
        );
        return;
    }

    let Some((owner_module, value)) = struct_utils::find_struct_constant_in_module_tree(
        graph,
        module_name,
        struct_name,
        constant_name,
    ) else {
        return;
    };

    enqueue_item(
        reachability,
        pending,
        &owner_module,
        ItemKey::StructConstant(struct_name.to_string(), constant_name.to_string()),
    );
    if let Some(owner) = graph.get(&owner_module) {
        collect_references_from_expression(
            graph,
            owner,
            value,
            &HashMap::new(),
            reachability,
            pending,
        );
    }
}

/// Add `item` to `module_name`'s reachable set and schedule it for expansion.
pub fn enqueue_item(
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
    module_name: &str,
    item: ItemKey,
) {
    let reach = reachability.entry(module_name.to_string()).or_default();
    if reach.items.insert(item.clone()) {
        pending.push_back((module_name.to_string(), item));
    }
}

/// Returns whether a top-level statement should remain after pruning.
pub fn is_statement_reachable(statement: &Statement, reach: &ModuleReachability) -> bool {
    match statement {
        Statement::FunctionDecl(function) => reach
            .items
            .contains(&ItemKey::Function(function.name.clone())),
        Statement::StructDecl(struct_decl) => is_struct_reachable(struct_decl, reach),
        // Nested statements are pruned with their containing function body.
        Statement::VariableDecl { .. }
        | Statement::Assignment { .. }
        | Statement::Conditional { .. }
        | Statement::Return(_)
        | Statement::Expr(_) => true,
    }
}

/// Returns whether a struct declaration (or any of its members) is reachable.
pub fn is_struct_reachable(struct_decl: &Struct, reach: &ModuleReachability) -> bool {
    reach
        .items
        .contains(&ItemKey::Struct(struct_decl.name.clone()))
        || struct_decl.methods.iter().any(|method| {
            reach.items.contains(&ItemKey::StructMethod(
                struct_decl.name.clone(),
                method.name.clone(),
            ))
        })
        || struct_decl.constants.iter().any(|(name, _)| {
            reach.items.contains(&ItemKey::StructConstant(
                struct_decl.name.clone(),
                name.clone(),
            ))
        })
}
