use std::collections::HashSet;

use crate::{
    expression::Expression, function::Function, module::Module,
    prune::module_graph::ModuleGraph, statement::Statement, structure::Struct,
};

/// Find a struct declaration in a module's body or exported symbols.
pub fn find_struct<'a>(module: &'a Module, name: &str) -> Option<&'a Struct> {
    module
        .body
        .statements
        .iter()
        .chain(module.symbols.iter().map(|symbol| &symbol.statement))
        .find_map(|statement| match statement {
            Statement::StructDecl(struct_decl) if struct_decl.name == name => Some(struct_decl),
            _ => None,
        })
}

/// Returns whether `module` declares a struct named `name`.
pub fn struct_exists(module: &Module, name: &str) -> bool {
    find_struct(module, name).is_some()
}

/// Find an instance method on a struct declared in `module`.
pub fn find_struct_method<'a>(
    module: &'a Module,
    struct_name: &str,
    method_name: &str,
) -> Option<&'a Function> {
    find_struct(module, struct_name).and_then(|struct_decl| {
        struct_decl
            .methods
            .iter()
            .find(|method| method.name == method_name)
    })
}

/// Find an associated constant initializer on a struct declared in `module`.
pub fn find_struct_constant<'a>(
    module: &'a Module,
    struct_name: &str,
    constant_name: &str,
) -> Option<&'a Expression> {
    find_struct(module, struct_name).and_then(|struct_decl| {
        struct_decl
            .constants
            .iter()
            .find_map(|(name, value)| (name == constant_name).then_some(value))
    })
}

/// Returns whether `struct_name` defines an associated constant named `constant_name`.
pub fn struct_has_constant(module: &Module, struct_name: &str, constant_name: &str) -> bool {
    find_struct_constant(module, struct_name, constant_name).is_some()
}

/// Returns whether `struct_name` defines a method named `method_name`.
pub fn struct_has_method(module: &Module, struct_name: &str, method_name: &str) -> bool {
    find_struct_method(module, struct_name, method_name).is_some()
}

/// Return the module that owns a struct type referenced from `module`.
///
/// Checks local declarations first, then walks `use` imports and their submodules.
pub fn struct_owner_module(
    graph: &ModuleGraph<'_>,
    module: &Module,
    struct_name: &str,
) -> String {
    if struct_exists(module, struct_name) {
        return module.name.clone();
    }

    for import in &module.imports {
        for item in &import.items {
            if item.name == struct_name || item.local == struct_name {
                return import.module.clone();
            }
        }

        if module_defines_struct(graph, &import.module, struct_name) {
            return import.module.clone();
        }
    }

    module.name.clone()
}

/// Returns whether `struct_name` is declared in `module_name` or any of its submodules.
pub fn module_defines_struct(
    graph: &ModuleGraph<'_>,
    module_name: &str,
    struct_name: &str,
) -> bool {
    let mut stack = vec![module_name.to_string()];
    let mut seen = HashSet::new();

    while let Some(current) = stack.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }

        if let Some(module) = graph.get(&current)
            && struct_exists(module, struct_name)
        {
            return true;
        }

        if let Some(module) = graph.get(&current) {
            for child in graph.child_modules(&module.name) {
                stack.push((*child).to_string());
            }
        }
    }

    false
}

/// Search `module_name` and its submodules for a struct method definition.
pub fn find_struct_method_in_module_tree<'a>(
    graph: &ModuleGraph<'a>,
    module_name: &str,
    struct_name: &str,
    method_name: &str,
) -> Option<(String, &'a Function)> {
    let mut stack = vec![module_name.to_string()];
    let mut seen = HashSet::new();

    while let Some(current) = stack.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }

        let Some(module) = graph.get(&current) else {
            continue;
        };

        if let Some(method) = find_struct_method(module, struct_name, method_name) {
            return Some((current, method));
        }

        for child in graph.child_modules(&current) {
            stack.push((*child).to_string());
        }
    }

    None
}

/// Search `module_name` and its submodules for a struct associated constant.
pub fn find_struct_constant_in_module_tree<'a>(
    graph: &ModuleGraph<'a>,
    module_name: &str,
    struct_name: &str,
    constant_name: &str,
) -> Option<(String, &'a Expression)> {
    let mut stack = vec![module_name.to_string()];
    let mut seen = HashSet::new();

    while let Some(current) = stack.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }

        let Some(module) = graph.get(&current) else {
            continue;
        };

        if let Some(value) = find_struct_constant(module, struct_name, constant_name) {
            return Some((current, value));
        }

        for child in graph.child_modules(&current) {
            stack.push((*child).to_string());
        }
    }

    None
}
