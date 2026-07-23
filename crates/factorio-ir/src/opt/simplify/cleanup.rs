use crate::{expression::Expression, literal::Literal, operator::Operator, statement::Statement};

/// `local x = nil; x = e` -> `local x = e` (no intervening use of `x`).
pub(super) fn collapse_nil_inits(stmts: &[Statement]) -> Vec<Statement> {
    let mut out = Vec::with_capacity(stmts.len());
    let mut i = 0;
    while i < stmts.len() {
        if let Statement::VariableDecl {
            name,
            ty,
            source_type,
            value: Expression::Literal(Literal::Nil),
        } = &stmts[i]
            && let Some(Statement::Assignment {
                target: Expression::Identifier(dest),
                value,
            }) = stmts.get(i + 1)
            && dest == name
        {
            out.push(Statement::VariableDecl {
                name: name.clone(),
                ty: ty.clone(),
                source_type: source_type.clone(),
                value: value.clone(),
            });
            i += 2;
            continue;
        }
        out.push(stmts[i].clone());
        i += 1;
    }
    out
}

/// `if v ~= nil then local x = v; return x end` -> `return v` (identity binds).
pub(super) fn simplify_identity_binds(block: &mut Vec<Statement>) {
    if let [
        Statement::VariableDecl { name, value, .. },
        Statement::Return(Some(Expression::Identifier(ret))),
    ] = block.as_slice()
        && name == ret
    {
        *block = vec![Statement::Return(Some(value.clone()))];
        return;
    }
    if let [
        Statement::VariableDecl { name, value, .. },
        Statement::Assignment {
            target,
            value: Expression::Identifier(src),
        },
    ] = block.as_slice()
        && name == src
    {
        *block = vec![Statement::Assignment {
            target: target.clone(),
            value: value.clone(),
        }];
    }
}

/// `if v ~= nil then ... else if v == nil then BODY end end` -> else BODY.
pub(super) fn drop_redundant_nil_else(condition: &Expression, else_block: &mut Vec<Statement>) {
    let Some(name) = super::unwrap_or::ne_nil_ident(condition) else {
        return;
    };
    if let [
        Statement::Conditional {
            condition: inner_cond,
            then_block,
            else_block: inner_else,
        },
    ] = else_block.as_slice()
        && is_eq_nil(inner_cond, &name)
        && inner_else.is_empty()
    {
        *else_block = then_block.clone();
    }
}

fn is_eq_nil(condition: &Expression, name: &str) -> bool {
    matches!(
        condition,
        Expression::BinaryOp {
            lhs,
            op: Operator::Eq,
            rhs,
        } if matches!(lhs.as_ref(), Expression::Identifier(id) if id == name)
            && matches!(rhs.as_ref(), Expression::Literal(Literal::Nil))
    )
}
