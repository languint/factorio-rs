use crate::{expression::Expression, statement::Statement, r#type::Type};

use super::{
    expand::{ValueSink, expand_value_binding},
    is_simple_assign_target,
};

pub(super) fn extract_mid_expr_hoists(
    statement: &mut Statement,
    prefix: &mut Vec<Statement>,
    counter: &mut u32,
) {
    match statement {
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => extract_children_hoists(value, prefix, counter),
        Statement::Assignment { target, value } => {
            extract_children_hoists(target, prefix, counter);
            if is_simple_assign_target(target) && is_hoistable_value(value) {
                extract_children_hoists(value, prefix, counter);
            } else {
                extract_nested_hoists(value, prefix, counter);
            }
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            extract_nested_hoists(condition, prefix, counter);
            for s in then_block.iter_mut().chain(else_block.iter_mut()) {
                extract_mid_expr_hoists(s, prefix, counter);
            }
        }
        Statement::ForIn { iter, body, .. } => {
            extract_nested_hoists(iter, prefix, counter);
            for s in body {
                extract_mid_expr_hoists(s, prefix, counter);
            }
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            extract_nested_hoists(start, prefix, counter);
            extract_nested_hoists(limit, prefix, counter);
            for s in body {
                extract_mid_expr_hoists(s, prefix, counter);
            }
        }
        Statement::While { condition, body } => {
            extract_nested_hoists(condition, prefix, counter);
            for s in body {
                extract_mid_expr_hoists(s, prefix, counter);
            }
        }
        Statement::FunctionDecl(_)
        | Statement::StructDecl(_)
        | Statement::EnumDecl(_)
        | Statement::Return(None)
        | Statement::Continue
        | Statement::Break
        | Statement::RawLua { .. } => {}
    }
}

fn extract_nested_hoists(expr: &mut Expression, prefix: &mut Vec<Statement>, counter: &mut u32) {
    if is_hoistable_value(expr) {
        hoist_to_temp(expr, prefix, counter);
        return;
    }
    extract_children_hoists(expr, prefix, counter);
}

fn extract_children_hoists(expr: &mut Expression, prefix: &mut Vec<Statement>, counter: &mut u32) {
    match expr {
        Expression::Literal(_)
        | Expression::Identifier(_)
        | Expression::QualifiedPath { .. }
        | Expression::Closure { .. } => {}
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => {
            extract_nested_hoists(base, prefix, counter);
        }
        Expression::Call { func, args } => {
            let hoist_func = !args.is_empty()
                || !matches!(
                    func.as_ref(),
                    Expression::Closure { params, .. } if params.is_empty()
                );
            for arg in args.iter_mut() {
                extract_nested_hoists(arg, prefix, counter);
            }
            if hoist_func {
                extract_nested_hoists(func, prefix, counter);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            extract_nested_hoists(receiver, prefix, counter);
            for arg in args {
                extract_nested_hoists(arg, prefix, counter);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                extract_nested_hoists(value, prefix, counter);
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            extract_nested_hoists(lhs, prefix, counter);
            extract_nested_hoists(rhs, prefix, counter);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                extract_nested_hoists(part, prefix, counter);
            }
        }
        Expression::Index { base, key } => {
            extract_nested_hoists(base, prefix, counter);
            extract_nested_hoists(key, prefix, counter);
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            extract_nested_hoists(condition, prefix, counter);
            extract_nested_hoists(then_expr, prefix, counter);
            extract_nested_hoists(else_expr, prefix, counter);
        }
    }
}

fn is_hoistable_value(expr: &Expression) -> bool {
    match expr {
        Expression::If { .. } => true,
        Expression::Call { func, args } if args.is_empty() => {
            matches!(func.as_ref(), Expression::Closure { params, .. } if params.is_empty())
        }
        _ => false,
    }
}

fn hoist_to_temp(expr: &mut Expression, prefix: &mut Vec<Statement>, counter: &mut u32) {
    let name = format!("__h{counter}");
    *counter += 1;
    let value = std::mem::replace(expr, Expression::Identifier(name.clone()));
    prefix.extend(expand_value_binding(
        ValueSink::Local {
            name,
            ty: Type::Void,
            source_type: None,
        },
        value,
    ));
}
