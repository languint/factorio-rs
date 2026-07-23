use crate::expression::Expression;

use super::optimize_block;

pub(super) fn optimize_expression(expr: &mut Expression) {
    match expr {
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {}
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => optimize_expression(base),
        Expression::Call { func, args } => {
            optimize_expression(func);
            for arg in args {
                optimize_expression(arg);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            optimize_expression(receiver);
            for arg in args {
                optimize_expression(arg);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                optimize_expression(value);
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            optimize_expression(lhs);
            optimize_expression(rhs);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                optimize_expression(part);
            }
        }
        Expression::Index { base, key } => {
            optimize_expression(base);
            optimize_expression(key);
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            optimize_expression(condition);
            optimize_expression(then_expr);
            optimize_expression(else_expr);
        }
        Expression::Closure { body, .. } => optimize_block(body),
    }
}
