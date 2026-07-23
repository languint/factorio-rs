use crate::{
    expression::Expression, literal::Literal, operator::Operator, statement::Statement,
    r#type::Type,
};

pub(super) fn try_simplify_result_unwrap_or(
    stmts: &[Statement],
) -> Option<(usize, Vec<Statement>)> {
    let Statement::VariableDecl {
        name: dest,
        ty,
        source_type,
        value: Expression::Literal(Literal::Nil),
    } = &stmts[0]
    else {
        return None;
    };

    if stmts.len() >= 3
        && let Statement::VariableDecl {
            name: tmp,
            value: recv,
            ..
        } = &stmts[1]
        && let Statement::Conditional {
            condition,
            then_block,
            else_block,
        } = &stmts[2]
        && is_result_ok_condition(condition, tmp)
        && is_single_assign_ok_field(then_block, dest, tmp)
        && let Some(default) = single_assign_value(else_block, dest)
    {
        // Keep the bind-once temp so side-effecting receivers stay single-eval.
        let mut out = vec![Statement::VariableDecl {
            name: tmp.clone(),
            ty: Type::Void,
            source_type: None,
            value: recv.clone(),
        }];
        out.extend(rewrite_result_unwrap_or(
            dest,
            ty,
            source_type.as_ref(),
            tmp,
            default,
        ));
        return Some((3, out));
    }

    if stmts.len() >= 2
        && let Statement::Conditional {
            condition,
            then_block,
            else_block,
        } = &stmts[1]
        && let Some(src) = result_ok_condition_ident(condition)
        && is_single_assign_ok_field(then_block, dest, &src)
        && let Some(default) = single_assign_value(else_block, dest)
    {
        return Some((
            2,
            rewrite_result_unwrap_or(dest, ty, source_type.as_ref(), &src, default),
        ));
    }

    None
}

fn rewrite_result_unwrap_or(
    dest: &str,
    ty: &Type,
    source_type: Option<&String>,
    recv_name: &str,
    default: Expression,
) -> Vec<Statement> {
    let recv = Expression::Identifier(recv_name.to_string());
    let err_check = Expression::BinaryOp {
        lhs: Box::new(Expression::FieldAccess {
            base: Box::new(recv.clone()),
            field: "err".to_string(),
        }),
        op: Operator::Ne,
        rhs: Box::new(Expression::Literal(Literal::Nil)),
    };
    vec![
        Statement::VariableDecl {
            name: dest.to_string(),
            ty: ty.clone(),
            source_type: source_type.cloned(),
            value: Expression::FieldAccess {
                base: Box::new(recv),
                field: "ok".to_string(),
            },
        },
        Statement::Conditional {
            condition: err_check,
            then_block: vec![Statement::Assignment {
                target: Expression::Identifier(dest.to_string()),
                value: default,
            }],
            else_block: vec![],
        },
    ]
}

fn is_result_ok_condition(condition: &Expression, name: &str) -> bool {
    matches!(
        condition,
        Expression::BinaryOp {
            lhs,
            op: Operator::Eq,
            rhs,
        } if matches!(
            lhs.as_ref(),
            Expression::FieldAccess { base, field }
                if field == "err"
                    && matches!(base.as_ref(), Expression::Identifier(id) if id == name)
        ) && matches!(rhs.as_ref(), Expression::Literal(Literal::Nil))
    )
}

fn result_ok_condition_ident(condition: &Expression) -> Option<String> {
    match condition {
        Expression::BinaryOp {
            lhs,
            op: Operator::Eq,
            rhs,
        } if matches!(rhs.as_ref(), Expression::Literal(Literal::Nil)) => match lhs.as_ref() {
            Expression::FieldAccess { base, field } if field == "err" => match base.as_ref() {
                Expression::Identifier(id) => Some(id.clone()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn is_single_assign_ok_field(block: &[Statement], dest: &str, recv: &str) -> bool {
    matches!(
        block,
        [Statement::Assignment {
            target: Expression::Identifier(t),
            value: Expression::FieldAccess { base, field },
        }] if t == dest
            && field == "ok"
            && matches!(base.as_ref(), Expression::Identifier(id) if id == recv)
    )
}

/// `local n = nil; [local tmp = recv;] if tmp ~= nil then n = tmp else n = d end`
/// -> `local n = recv; if n == nil then n = d end`
pub(super) fn try_simplify_unwrap_or(stmts: &[Statement]) -> Option<(usize, Vec<Statement>)> {
    let Statement::VariableDecl {
        name: dest,
        ty,
        source_type,
        value: Expression::Literal(Literal::Nil),
    } = &stmts[0]
    else {
        return None;
    };

    // Pattern with bind-once temp: local n = nil; local tmp = recv; if ...
    if stmts.len() >= 3
        && let Statement::VariableDecl {
            name: tmp,
            value: recv,
            ..
        } = &stmts[1]
        && let Statement::Conditional {
            condition,
            then_block,
            else_block,
        } = &stmts[2]
        && is_ne_nil(condition, tmp)
        && is_single_assign_ident(then_block, dest, tmp)
        && let Some(default) = single_assign_value(else_block, dest)
    {
        return Some((
            3,
            rewrite_unwrap_or(dest, ty, source_type.as_ref(), recv.clone(), default),
        ));
    }

    // Pattern without temp: local n = nil; if x ~= nil then n = x else n = d end
    if stmts.len() >= 2
        && let Statement::Conditional {
            condition,
            then_block,
            else_block,
        } = &stmts[1]
        && let Some(src) = ne_nil_ident(condition)
        && is_single_assign_ident(then_block, dest, &src)
        && let Some(default) = single_assign_value(else_block, dest)
    {
        return Some((
            2,
            rewrite_unwrap_or(
                dest,
                ty,
                source_type.as_ref(),
                Expression::Identifier(src),
                default,
            ),
        ));
    }

    None
}

fn rewrite_unwrap_or(
    dest: &str,
    ty: &Type,
    source_type: Option<&String>,
    value: Expression,
    default: Expression,
) -> Vec<Statement> {
    vec![
        Statement::VariableDecl {
            name: dest.to_string(),
            ty: ty.clone(),
            source_type: source_type.cloned(),
            value,
        },
        Statement::Conditional {
            condition: Expression::BinaryOp {
                lhs: Box::new(Expression::Identifier(dest.to_string())),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::Nil)),
            },
            then_block: vec![Statement::Assignment {
                target: Expression::Identifier(dest.to_string()),
                value: default,
            }],
            else_block: vec![],
        },
    ]
}

fn is_ne_nil(condition: &Expression, name: &str) -> bool {
    matches!(
        condition,
        Expression::BinaryOp {
            lhs,
            op: Operator::Ne,
            rhs,
        } if matches!(lhs.as_ref(), Expression::Identifier(id) if id == name)
            && matches!(rhs.as_ref(), Expression::Literal(Literal::Nil))
    )
}

pub(super) fn ne_nil_ident(condition: &Expression) -> Option<String> {
    match condition {
        Expression::BinaryOp {
            lhs,
            op: Operator::Ne,
            rhs,
        } if matches!(rhs.as_ref(), Expression::Literal(Literal::Nil)) => match lhs.as_ref() {
            Expression::Identifier(id) => Some(id.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn is_single_assign_ident(block: &[Statement], dest: &str, src: &str) -> bool {
    matches!(
        block,
        [Statement::Assignment {
            target: Expression::Identifier(t),
            value: Expression::Identifier(v),
        }] if t == dest && v == src
    )
}

fn single_assign_value(block: &[Statement], dest: &str) -> Option<Expression> {
    match block {
        [
            Statement::Assignment {
                target: Expression::Identifier(t),
                value,
            },
        ] if t == dest => Some(value.clone()),
        _ => None,
    }
}
