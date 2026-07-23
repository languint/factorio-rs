use std::collections::{HashMap, VecDeque};

use crate::{
    block::Block,
    expression::Expression,
    function::Function,
    module::Module,
    prune::{
        items::function_exists,
        module_graph::{ItemKey, ModuleGraph},
        reachability::{ModuleReachability, enqueue_item},
        struct_utils,
    },
    statement::Statement,
};

use super::resolve::{
    infer_struct_type_from_expression, queue_struct_member, resolve_call_target, resolve_import,
    resolve_struct_member_reference, type_name_from_source,
};

/// Record every item referenced from `function`'s body into the reachability worklist.
pub fn collect_references_from_function(
    graph: &ModuleGraph<'_>,
    module: &Module,
    function: &Function,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    let mut locals = HashMap::new();
    for parameter in &function.params {
        if parameter.name != "self"
            && let Some(source_type) = &parameter.source_type
            && let Some(struct_name) = type_name_from_source(source_type)
        {
            locals.insert(parameter.name.clone(), struct_name);
        }
    }

    collect_references_from_block(
        graph,
        module,
        &function.body,
        &mut locals,
        reachability,
        pending,
    );
}

/// Walk all statements in `block`, extending `locals` and the worklist.
fn collect_references_from_block(
    graph: &ModuleGraph<'_>,
    module: &Module,
    block: &Block,
    locals: &mut HashMap<String, String>,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    for statement in &block.statements {
        collect_references_from_statement(graph, module, statement, locals, reachability, pending);
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_references_from_conditional(
    graph: &ModuleGraph<'_>,
    module: &Module,
    locals: &mut HashMap<String, String>,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
    condition: &Expression,
    then_block: &Vec<Statement>,
    else_block: &Vec<Statement>,
) {
    collect_references_from_expression(graph, module, condition, locals, reachability, pending);
    for statement in then_block {
        collect_references_from_statement(graph, module, statement, locals, reachability, pending);
    }
    for statement in else_block {
        collect_references_from_statement(graph, module, statement, locals, reachability, pending);
    }
}

/// Walk one statement, updating `locals` and enqueueing referenced items.
fn collect_references_from_statement(
    graph: &ModuleGraph<'_>,
    module: &Module,
    statement: &Statement,
    locals: &mut HashMap<String, String>,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    match statement {
        Statement::FunctionDecl(function) => {
            collect_references_from_function(graph, module, function, reachability, pending);
        }
        Statement::VariableDecl {
            name,
            source_type,
            value,
            ..
        } => {
            if let Some(source_type) = source_type
                && let Some(struct_name) = type_name_from_source(source_type)
            {
                locals.insert(name.clone(), struct_name);
            } else if let Some(struct_name) = infer_struct_type_from_expression(value) {
                locals.insert(name.clone(), struct_name);
            }
            collect_references_from_expression(graph, module, value, locals, reachability, pending);
        }
        Statement::Assignment { target, value } => {
            collect_references_from_expression(
                graph,
                module,
                target,
                locals,
                reachability,
                pending,
            );
            collect_references_from_expression(graph, module, value, locals, reachability, pending);
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => collect_references_from_conditional(
            graph,
            module,
            locals,
            reachability,
            pending,
            condition,
            then_block,
            else_block,
        ),
        Statement::Return(value) => {
            if let Some(value) = value {
                collect_references_from_expression(
                    graph,
                    module,
                    value,
                    locals,
                    reachability,
                    pending,
                );
            }
        }
        Statement::Expr(expression) => {
            collect_references_from_expression(
                graph,
                module,
                expression,
                locals,
                reachability,
                pending,
            );
        }
        Statement::ForIn { iter, body, .. } => {
            collect_references_from_expression(graph, module, iter, locals, reachability, pending);
            collect_references_from_stmts(graph, module, body, locals, reachability, pending);
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            collect_references_from_expression(graph, module, start, locals, reachability, pending);
            collect_references_from_expression(graph, module, limit, locals, reachability, pending);
            collect_references_from_stmts(graph, module, body, locals, reachability, pending);
        }
        Statement::While { condition, body } => {
            collect_references_from_expression(
                graph,
                module,
                condition,
                locals,
                reachability,
                pending,
            );
            collect_references_from_stmts(graph, module, body, locals, reachability, pending);
        }
        Statement::StructDecl(_)
        | Statement::EnumDecl(_)
        | Statement::Continue
        | Statement::Break => {}
    }
}

fn collect_references_from_stmts(
    graph: &ModuleGraph<'_>,
    module: &Module,
    body: &[Statement],
    locals: &mut HashMap<String, String>,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    for statement in body {
        collect_references_from_statement(graph, module, statement, locals, reachability, pending);
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_references_from_method_call(
    graph: &ModuleGraph<'_>,
    module: &Module,
    locals: &HashMap<String, String>,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
    receiver: &Expression,
    method: &str,
    args: &[Expression],
) {
    if let Expression::Identifier(name) = receiver {
        if let Some((target_module, struct_name)) = resolve_import(module, name) {
            enqueue_item(
                reachability,
                pending,
                &target_module,
                ItemKey::StructMethod(struct_name, method.to_owned()),
            );
        } else if let Some(struct_name) = locals.get(name) {
            let owner = struct_utils::struct_owner_module(graph, module, struct_name);
            enqueue_item(
                reachability,
                pending,
                &owner,
                ItemKey::StructMethod(struct_name.clone(), method.to_owned()),
            );
        }
    } else {
        collect_references_from_expression(graph, module, receiver, locals, reachability, pending);
    }
    for arg in args {
        collect_references_from_expression(graph, module, arg, locals, reachability, pending);
    }
}

fn collect_references_from_field_access(
    graph: &ModuleGraph<'_>,
    module: &Module,
    locals: &HashMap<String, String>,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
    base: &Expression,
    field: &str,
) {
    if let Expression::Identifier(name) = base {
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
                ItemKey::StructMethod(struct_name, field.to_owned()),
            );
        } else if let Some(struct_name) = locals.get(name) {
            queue_struct_member(graph, module, struct_name, field, reachability, pending);
        } else {
            queue_struct_member(graph, module, name, field, reachability, pending);
        }
    } else {
        collect_references_from_expression(graph, module, base, locals, reachability, pending);
    }
}

/// Walk an expression tree and enqueue every referenced function, struct, or member.
#[allow(clippy::too_many_lines)]
pub fn collect_references_from_expression(
    graph: &ModuleGraph<'_>,
    module: &Module,
    expression: &Expression,
    locals: &HashMap<String, String>,
    reachability: &mut HashMap<String, ModuleReachability>,
    pending: &mut VecDeque<(String, ItemKey)>,
) {
    match expression {
        Expression::Literal(_) => {}
        Expression::Identifier(name) => {
            // Function names used as values (e.g. `commands.add_command(..., greet)`)
            // must keep the referenced function alive under dead-code pruning.
            if function_exists(module, name) {
                enqueue_item(
                    reachability,
                    pending,
                    &module.name,
                    ItemKey::Function(name.clone()),
                );
            } else if struct_utils::struct_exists(module, name) {
                enqueue_item(
                    reachability,
                    pending,
                    &module.name,
                    ItemKey::Struct(name.clone()),
                );
            }
        }
        Expression::QualifiedPath { segments } => {
            resolve_struct_member_reference(graph, module, segments, reachability, pending);
        }
        Expression::FieldAccess { base, field } => collect_references_from_field_access(
            graph,
            module,
            locals,
            reachability,
            pending,
            base,
            field,
        ),
        Expression::Call { func, args } => {
            resolve_call_target(graph, module, func, locals, reachability, pending);
            for arg in args {
                collect_references_from_expression(
                    graph,
                    module,
                    arg,
                    locals,
                    reachability,
                    pending,
                );
            }
        }
        Expression::MethodCall {
            receiver,
            method,
            args,
            ..
        } => collect_references_from_method_call(
            graph,
            module,
            locals,
            reachability,
            pending,
            receiver,
            method,
            args,
        ),
        Expression::FatPointer { data, .. } => {
            collect_references_from_expression(graph, module, data, locals, reachability, pending);
        }
        Expression::DynMethodCall { receiver, args, .. } => {
            collect_references_from_expression(
                graph,
                module,
                receiver,
                locals,
                reachability,
                pending,
            );
            for arg in args {
                collect_references_from_expression(
                    graph,
                    module,
                    arg,
                    locals,
                    reachability,
                    pending,
                );
            }
        }
        Expression::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_references_from_expression(
                    graph,
                    module,
                    value,
                    locals,
                    reachability,
                    pending,
                );
            }
        }
        Expression::EnumLiteral {
            enum_name, fields, ..
        } => {
            enqueue_item(
                reachability,
                pending,
                &module.name,
                ItemKey::Struct(enum_name.clone()),
            );
            for (_, value) in fields {
                collect_references_from_expression(
                    graph,
                    module,
                    value,
                    locals,
                    reachability,
                    pending,
                );
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            collect_references_from_expression(graph, module, lhs, locals, reachability, pending);
            collect_references_from_expression(graph, module, rhs, locals, reachability, pending);
        }
        Expression::FormatConcat { parts } => {
            for part in parts {
                collect_references_from_expression(
                    graph,
                    module,
                    part,
                    locals,
                    reachability,
                    pending,
                );
            }
        }
        Expression::Array { elements } => {
            for element in elements {
                collect_references_from_expression(
                    graph,
                    module,
                    element,
                    locals,
                    reachability,
                    pending,
                );
            }
        }
        Expression::Index { base, key } => {
            collect_references_from_expression(graph, module, base, locals, reachability, pending);
            collect_references_from_expression(graph, module, key, locals, reachability, pending);
        }
        Expression::Not(inner) | Expression::Len(inner) => {
            collect_references_from_expression(graph, module, inner, locals, reachability, pending);
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_references_from_expression(
                graph,
                module,
                condition,
                locals,
                reachability,
                pending,
            );
            collect_references_from_expression(
                graph,
                module,
                then_expr,
                locals,
                reachability,
                pending,
            );
            collect_references_from_expression(
                graph,
                module,
                else_expr,
                locals,
                reachability,
                pending,
            );
        }
        Expression::Closure { body, .. } => {
            let mut closure_locals = locals.clone();
            collect_references_from_block(
                graph,
                module,
                body,
                &mut closure_locals,
                reachability,
                pending,
            );
        }
    }
}
