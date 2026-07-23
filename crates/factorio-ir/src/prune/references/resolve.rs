use std::collections::{HashMap, VecDeque};

use crate::{
    expression::Expression,
    module::Module,
    prune::{
        items::function_exists,
        module_graph::{ItemKey, ModuleGraph},
        reachability::{ModuleReachability, enqueue_item},
        struct_utils,
    },
    structure::Struct,
};

use super::walk::collect_references_from_expression;

/// Resolve the callee expression of a call and enqueue the target item.
pub(super) fn resolve_call_target(
    graph: &ModuleGraph<'_>,
    module: &Module,
    func: &Expression,
    locals: &HashMap<String, String>,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    match func {
        Expression::Identifier(name) => {
            enqueue_item(
                reachability,
                pending,
                &module.name,
                ItemKey::Function(name.clone()),
            );
        }
        Expression::FieldAccess { base, field } => {
            if let Expression::Identifier(name) = base.as_ref() {
                if let Some((target_module, struct_name)) = resolve_import(module, name) {
                    enqueue_item(
                        reachability,
                        pending,
                        &target_module,
                        ItemKey::Struct(struct_name.clone()),
                    );
                    enqueue_item(
                        reachability,
                        pending,
                        &target_module,
                        ItemKey::StructMethod(struct_name, field.clone()),
                    );
                } else if let Some(struct_name) = locals.get(name) {
                    let owner = struct_utils::struct_owner_module(graph, module, struct_name);
                    enqueue_item(
                        reachability,
                        pending,
                        &owner,
                        ItemKey::StructMethod(struct_name.clone(), field.clone()),
                    );
                } else {
                    queue_struct_member(graph, module, name, field, reachability, pending);
                }
            } else {
                collect_references_from_expression(
                    graph,
                    module,
                    base,
                    locals,
                    reachability,
                    pending,
                );
            }
        }
        Expression::QualifiedPath { segments } => {
            if let Some((target_module, rest)) = resolve_module_path(module, segments) {
                // A call like `module::function(...)` resolves to a plain function in the
                // imported module when there is exactly one trailing segment.
                if rest.len() == 1 {
                    enqueue_item(
                        reachability,
                        pending,
                        &target_module,
                        ItemKey::Function(rest[0].clone()),
                    );
                } else {
                    enqueue_import_path(reachability, pending, &target_module, &rest);
                }
            } else if segments.len() >= 2 {
                let struct_name = segments[0].clone();
                let member = segments[1].clone();
                if function_exists(module, &member) && segments.len() == 2 {
                    enqueue_item(
                        reachability,
                        pending,
                        &module.name,
                        ItemKey::Function(member),
                    );
                } else {
                    queue_struct_member(
                        graph,
                        module,
                        &struct_name,
                        &member,
                        reachability,
                        pending,
                    );
                }
            }
        }
        _ => collect_references_from_expression(graph, module, func, locals, reachability, pending),
    }
}

/// Resolve a qualified path used as a value (e.g. `MyPlayer.DEFAULT_HEALTH`).
pub(super) fn resolve_struct_member_reference(
    graph: &ModuleGraph<'_>,
    module: &Module,
    segments: &[String],
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    if segments.is_empty() {
        return;
    }

    if segments.len() == 1 {
        let name = &segments[0];
        if let Some((target_module, struct_name)) = resolve_import(module, name) {
            enqueue_item(
                reachability,
                pending,
                &target_module,
                ItemKey::Struct(struct_name),
            );
        } else if struct_utils::struct_exists(module, name) {
            enqueue_item(
                reachability,
                pending,
                &module.name,
                ItemKey::Struct(name.clone()),
            );
        } else if function_exists(module, name) {
            enqueue_item(
                reachability,
                pending,
                &module.name,
                ItemKey::Function(name.clone()),
            );
        }
        return;
    }

    let first = &segments[0];
    if let Some((target_module, rest)) = resolve_module_path(module, segments) {
        enqueue_import_path(reachability, pending, &target_module, &rest);
        return;
    }

    if let Some((target_module, struct_name)) = resolve_import(module, first) {
        enqueue_item(
            reachability,
            pending,
            &target_module,
            ItemKey::Struct(struct_name.clone()),
        );
        enqueue_item(
            reachability,
            pending,
            &target_module,
            ItemKey::StructMethod(struct_name, segments[1].clone()),
        );
        return;
    }

    queue_struct_member(graph, module, first, &segments[1], reachability, pending);
}

/// Enqueue a struct member, searching child modules for submodule impl extensions.
pub(super) fn queue_struct_member(
    graph: &ModuleGraph<'_>,
    module: &Module,
    struct_name: &str,
    member: &str,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    enqueue_item(
        reachability,
        pending,
        &module.name,
        ItemKey::Struct(struct_name.to_string()),
    );

    if struct_utils::struct_has_constant(module, struct_name, member) {
        enqueue_item(
            reachability,
            pending,
            &module.name,
            ItemKey::StructConstant(struct_name.to_string(), member.to_string()),
        );
        return;
    }

    if struct_utils::struct_has_method(module, struct_name, member) {
        enqueue_item(
            reachability,
            pending,
            &module.name,
            ItemKey::StructMethod(struct_name.to_string(), member.to_string()),
        );
        return;
    }

    if function_exists(module, member) {
        enqueue_item(
            reachability,
            pending,
            &module.name,
            ItemKey::Function(member.to_string()),
        );
        return;
    }

    for child in graph.child_modules(&module.name) {
        if let Some(child_module) = graph.get(child)
            && child_module.is_imported_type_extension(&Struct {
                name: struct_name.to_string(),
                fields: vec![],
                constants: vec![],
                methods: vec![],
                doc: None,
                debug: None,
            })
        {
            if struct_utils::struct_has_constant(child_module, struct_name, member) {
                enqueue_item(
                    reachability,
                    pending,
                    &child_module.name,
                    ItemKey::StructConstant(struct_name.to_string(), member.to_string()),
                );
            } else if struct_utils::struct_has_method(child_module, struct_name, member) {
                enqueue_item(
                    reachability,
                    pending,
                    &child_module.name,
                    ItemKey::StructMethod(struct_name.to_string(), member.to_string()),
                );
            }
        }
    }
}

/// Map a `use`-imported local name to its source module and exported item name.
pub(super) fn resolve_import(module: &Module, local: &str) -> Option<(String, String)> {
    for import in &module.imports {
        for item in &import.items {
            if item.local == local {
                return Some((import.module.clone(), item.name.clone()));
            }
        }
    }
    None
}

/// Map a rewritten `crate::`-path prefix (`shared_player.MyPlayer.new`) to its module.
fn resolve_module_path(module: &Module, segments: &[String]) -> Option<(String, Vec<String>)> {
    if segments.is_empty() {
        return None;
    }

    let import = module
        .imports
        .iter()
        .find(|import| import.local == segments[0])?;
    Some((import.module.clone(), segments[1..].to_vec()))
}

/// Enqueue a struct (and optional method) referenced through a module require local.
fn enqueue_import_path(
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
    target_module: &str,
    rest: &[String],
) {
    if rest.is_empty() {
        return;
    }

    let struct_name = rest[0].clone();
    enqueue_item(
        reachability,
        pending,
        target_module,
        ItemKey::Struct(struct_name.clone()),
    );

    if rest.len() >= 2 {
        enqueue_item(
            reachability,
            pending,
            target_module,
            ItemKey::StructMethod(struct_name, rest[1].clone()),
        );
    }
}

/// Infer a struct type name from a constructor or method-call initializer expression.
pub(super) fn infer_struct_type_from_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Call { func, .. } => infer_struct_type_from_call(func),
        Expression::MethodCall { receiver, .. } | Expression::DynMethodCall { receiver, .. } => {
            match receiver.as_ref() {
                Expression::Identifier(name) => Some(name.clone()),
                Expression::QualifiedPath { segments } if !segments.is_empty() => {
                    Some(segments[0].clone())
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Extract the struct name from a call expression like `MyPlayer::new()`.
fn infer_struct_type_from_call(func: &Expression) -> Option<String> {
    match func {
        Expression::QualifiedPath { segments } if segments.len() >= 2 => {
            Some(segments[segments.len() - 2].clone())
        }
        Expression::FieldAccess { base, .. } => match base.as_ref() {
            Expression::Identifier(name) => Some(name.clone()),
            Expression::QualifiedPath { segments } if !segments.is_empty() => {
                Some(segments[0].clone())
            }
            _ => None,
        },
        _ => None,
    }
}

/// Take the last segment of a Rust type path (`shared::player::MyPlayer` -> `MyPlayer`).
pub(super) fn type_name_from_source(source_type: &str) -> Option<String> {
    source_type
        .rsplit("::")
        .next()
        .map(str::to_string)
        .filter(|name| !name.is_empty())
}
