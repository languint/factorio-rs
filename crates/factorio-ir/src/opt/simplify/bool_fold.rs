use crate::{expression::Expression, literal::Literal, operator::Operator, statement::Statement};

/// `if c { true } else { false }` -> `c` (and swapped -> `not c`).
pub(super) fn fold_bool_if_expr(expr: &mut Expression) {
    match expr {
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            fold_bool_if_expr(condition);
            fold_bool_if_expr(then_expr);
            fold_bool_if_expr(else_expr);
            match (as_bool(then_expr), as_bool(else_expr)) {
                (Some(true), Some(false)) => *expr = condition.as_ref().clone(),
                (Some(false), Some(true)) => {
                    *expr = Expression::Not(Box::new(condition.as_ref().clone()));
                }
                _ => {}
            }
        }
        Expression::Not(base) => {
            fold_bool_if_expr(base);
            match base.as_ref() {
                Expression::Not(inner) => {
                    *expr = inner.as_ref().clone();
                    fold_bool_if_expr(expr);
                }
                Expression::BinaryOp { lhs, op, rhs } => {
                    if let Some(negated) = negate_comparison(*op) {
                        *expr = Expression::BinaryOp {
                            lhs: lhs.clone(),
                            op: negated,
                            rhs: rhs.clone(),
                        };
                    }
                }
                _ => {}
            }
        }
        Expression::FieldAccess { base, .. }
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => fold_bool_if_expr(base),
        Expression::Call { func, args } => {
            for arg in args.iter_mut() {
                fold_bool_if_expr(arg);
            }
            fold_bool_if_expr(func);
            if args.is_empty()
                && let Expression::Closure { params, body } = func.as_ref()
                && params.is_empty()
                && let [Statement::Return(Some(value))] = body.statements.as_slice()
            {
                *expr = value.clone();
                fold_bool_if_expr(expr);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            fold_bool_if_expr(receiver);
            for arg in args {
                fold_bool_if_expr(arg);
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            fold_bool_if_expr(lhs);
            fold_bool_if_expr(rhs);
        }
        Expression::Index { base, key } => {
            fold_bool_if_expr(base);
            fold_bool_if_expr(key);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                fold_bool_if_expr(part);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                fold_bool_if_expr(value);
            }
        }
        Expression::Closure { body, .. } => {
            let statements = std::mem::take(&mut body.statements);
            body.statements = super::simplify_statements(&statements);
        }
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {}
    }
}

/// `if c then return true else return false end` -> `return c`
/// (and assign / `not` variants).
pub(super) fn try_fold_bool_conditional(statement: &Statement) -> Option<Statement> {
    let Statement::Conditional {
        condition,
        then_block,
        else_block,
    } = statement
    else {
        return None;
    };

    if let (Some(t), Some(f)) = (
        single_return_bool(then_block),
        single_return_bool(else_block),
    ) {
        return bool_pair_expr(condition, t, f).map(|value| Statement::Return(Some(value)));
    }

    if let (Some((dest_t, t)), Some((dest_f, f))) = (
        single_assign_bool(then_block),
        single_assign_bool(else_block),
    ) && dest_t == dest_f
    {
        return bool_pair_expr(condition, t, f).map(|value| Statement::Assignment {
            target: Expression::Identifier(dest_t),
            value,
        });
    }

    None
}

fn bool_pair_expr(condition: &Expression, then_true: bool, else_true: bool) -> Option<Expression> {
    match (then_true, else_true) {
        (true, false) => Some(condition.clone()),
        (false, true) => Some(Expression::Not(Box::new(condition.clone()))),
        _ => None,
    }
}

const fn single_return_bool(block: &[Statement]) -> Option<bool> {
    match block {
        [Statement::Return(Some(expr))] => as_bool(expr),
        _ => None,
    }
}

fn single_assign_bool(block: &[Statement]) -> Option<(String, bool)> {
    match block {
        [
            Statement::Assignment {
                target: Expression::Identifier(name),
                value,
            },
        ] => as_bool(value).map(|b| (name.clone(), b)),
        _ => None,
    }
}

const fn as_bool(expr: &Expression) -> Option<bool> {
    match expr {
        Expression::Literal(Literal::Bool(b)) => Some(*b),
        _ => None,
    }
}

const fn negate_comparison(op: Operator) -> Option<Operator> {
    match op {
        Operator::Eq => Some(Operator::Ne),
        Operator::Ne => Some(Operator::Eq),
        Operator::Lt => Some(Operator::Ge),
        Operator::Le => Some(Operator::Gt),
        Operator::Gt => Some(Operator::Le),
        Operator::Ge => Some(Operator::Lt),
        _ => None,
    }
}

/// `flag == true` -> `flag`; `flag == false` -> `not flag`.
pub(super) fn fold_bool_cmp_condition(expr: &mut Expression) {
    match expr {
        Expression::BinaryOp {
            lhs,
            op: Operator::Eq,
            rhs,
        } => {
            fold_bool_cmp_condition(lhs);
            fold_bool_cmp_condition(rhs);
            match (as_bool(rhs), as_bool(lhs)) {
                (Some(true), _) => *expr = lhs.as_ref().clone(),
                (Some(false), _) => *expr = Expression::Not(Box::new(lhs.as_ref().clone())),
                (_, Some(true)) => *expr = rhs.as_ref().clone(),
                (_, Some(false)) => *expr = Expression::Not(Box::new(rhs.as_ref().clone())),
                _ => {}
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            fold_bool_cmp_condition(lhs);
            fold_bool_cmp_condition(rhs);
        }
        Expression::Not(inner)
        | Expression::Len(inner)
        | Expression::FieldAccess { base: inner, .. }
        | Expression::FatPointer { data: inner, .. } => fold_bool_cmp_condition(inner),
        Expression::Call { func, args } => {
            fold_bool_cmp_condition(func);
            for arg in args {
                fold_bool_cmp_condition(arg);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            fold_bool_cmp_condition(receiver);
            for arg in args {
                fold_bool_cmp_condition(arg);
            }
        }
        Expression::Index { base, key } => {
            fold_bool_cmp_condition(base);
            fold_bool_cmp_condition(key);
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            fold_bool_cmp_condition(condition);
            fold_bool_cmp_condition(then_expr);
            fold_bool_cmp_condition(else_expr);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                fold_bool_cmp_condition(part);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                fold_bool_cmp_condition(value);
            }
        }
        Expression::Closure { body, .. } => {
            for statement in &mut body.statements {
                if let Statement::Conditional { condition, .. } = statement {
                    fold_bool_cmp_condition(condition);
                }
            }
        }
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {}
    }
}
