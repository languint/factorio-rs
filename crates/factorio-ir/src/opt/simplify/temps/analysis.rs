use crate::{expression::Expression, statement::Statement};

pub(super) fn count_straight_line_reads(name: &str, stmts: &[Statement]) -> usize {
    stmts
        .iter()
        .map(|s| count_straight_line_reads_in_statement(s, name))
        .sum()
}

fn count_straight_line_reads_in_statement(statement: &Statement, name: &str) -> usize {
    match statement {
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => count_reads_in_expr(value, name),
        Statement::Assignment { target, value } => {
            count_reads_in_expr(target, name) + count_reads_in_expr(value, name)
        }
        Statement::Conditional { condition, .. } | Statement::While { condition, .. } => {
            count_reads_in_expr(condition, name)
        }
        Statement::ForIn { iter, .. } => count_reads_in_expr(iter, name),
        Statement::ForNumeric { start, limit, .. } => {
            count_reads_in_expr(start, name) + count_reads_in_expr(limit, name)
        }
        Statement::FunctionDecl(_)
        | Statement::StructDecl(_)
        | Statement::EnumDecl(_)
        | Statement::Return(None)
        | Statement::Continue
        | Statement::Break => 0,
    }
}

pub(super) fn is_compiler_temp(name: &str) -> bool {
    name.starts_with("__")
}

pub(super) const fn is_cheap_to_rematerialize(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. }
    )
}

pub(super) fn is_pure(expr: &Expression) -> bool {
    match expr {
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {
            true
        }
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => is_pure(base),
        Expression::BinaryOp { lhs, rhs, .. } => is_pure(lhs) && is_pure(rhs),
        Expression::Index { base, key } => is_pure(base) && is_pure(key),
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            parts.iter().all(is_pure)
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            fields.iter().all(|(_, value)| is_pure(value))
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => is_pure(condition) && is_pure(then_expr) && is_pure(else_expr),
        Expression::Call { .. }
        | Expression::MethodCall { .. }
        | Expression::DynMethodCall { .. }
        | Expression::Closure { .. } => false,
    }
}

pub(super) fn is_written(name: &str, stmts: &[Statement]) -> bool {
    for statement in stmts {
        match statement {
            Statement::Assignment {
                target: Expression::Identifier(n),
                ..
            } if n == name => return true,
            Statement::ForIn { var, body, .. } | Statement::ForNumeric { var, body, .. } => {
                if var == name || is_written(name, body) {
                    return true;
                }
            }
            Statement::Conditional {
                then_block,
                else_block,
                ..
            } => {
                if is_written(name, then_block) || is_written(name, else_block) {
                    return true;
                }
            }
            Statement::While { body, .. } => {
                if is_written(name, body) {
                    return true;
                }
            }
            Statement::FunctionDecl(function) => {
                if function.params.iter().any(|p| p.name == name)
                    || is_written(name, &function.body.statements)
                {
                    return true;
                }
            }
            Statement::StructDecl(struct_decl) => {
                for method in &struct_decl.methods {
                    if method.params.iter().any(|p| p.name == name)
                        || is_written(name, &method.body.statements)
                    {
                        return true;
                    }
                }
            }
            Statement::EnumDecl(enum_decl) => {
                for method in &enum_decl.methods {
                    if method.params.iter().any(|p| p.name == name)
                        || is_written(name, &method.body.statements)
                    {
                        return true;
                    }
                }
            }
            Statement::VariableDecl { name: n, value, .. } => {
                if n == name || expr_writes_ident(value, name) {
                    return true;
                }
            }
            Statement::Return(Some(expr)) | Statement::Expr(expr) => {
                if expr_writes_ident(expr, name) {
                    return true;
                }
            }
            Statement::Assignment { target, value } => {
                if expr_writes_ident(target, name) || expr_writes_ident(value, name) {
                    return true;
                }
            }
            Statement::Return(None) | Statement::Continue | Statement::Break => {}
        }
    }
    false
}

/// Closures / nested fns that assign the outer name count as writes for safety.
fn expr_writes_ident(expr: &Expression, name: &str) -> bool {
    match expr {
        Expression::Closure { params, body } => {
            if params.iter().any(|p| p == name) {
                return false;
            }
            is_written(name, &body.statements)
        }
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => expr_writes_ident(base, name),
        Expression::Call { func, args } => {
            expr_writes_ident(func, name) || args.iter().any(|a| expr_writes_ident(a, name))
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            expr_writes_ident(receiver, name) || args.iter().any(|a| expr_writes_ident(a, name))
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            expr_writes_ident(lhs, name) || expr_writes_ident(rhs, name)
        }
        Expression::Index { base, key } => {
            expr_writes_ident(base, name) || expr_writes_ident(key, name)
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            parts.iter().any(|p| expr_writes_ident(p, name))
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            fields.iter().any(|(_, v)| expr_writes_ident(v, name))
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_writes_ident(condition, name)
                || expr_writes_ident(then_expr, name)
                || expr_writes_ident(else_expr, name)
        }
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {
            false
        }
    }
}

pub(super) fn count_reads(name: &str, stmts: &[Statement]) -> usize {
    stmts
        .iter()
        .map(|s| count_reads_in_statement(s, name))
        .sum()
}

fn count_reads_in_statement(statement: &Statement, name: &str) -> usize {
    match statement {
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => count_reads_in_expr(value, name),
        Statement::Assignment { target, value } => {
            count_reads_in_expr(target, name) + count_reads_in_expr(value, name)
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            count_reads_in_expr(condition, name)
                + count_reads(name, then_block)
                + count_reads(name, else_block)
        }
        Statement::ForIn { iter, body, .. } => {
            count_reads_in_expr(iter, name) + count_reads(name, body)
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            count_reads_in_expr(start, name)
                + count_reads_in_expr(limit, name)
                + count_reads(name, body)
        }
        Statement::While { condition, body } => {
            count_reads_in_expr(condition, name) + count_reads(name, body)
        }
        Statement::FunctionDecl(function) => {
            if function.params.iter().any(|p| p.name == name) {
                0
            } else {
                count_reads(name, &function.body.statements)
            }
        }
        Statement::StructDecl(struct_decl) => struct_decl
            .methods
            .iter()
            .map(|m| {
                if m.params.iter().any(|p| p.name == name) {
                    0
                } else {
                    count_reads(name, &m.body.statements)
                }
            })
            .sum(),
        Statement::EnumDecl(enum_decl) => enum_decl
            .methods
            .iter()
            .map(|m| {
                if m.params.iter().any(|p| p.name == name) {
                    0
                } else {
                    count_reads(name, &m.body.statements)
                }
            })
            .sum(),
        Statement::Return(None) | Statement::Continue | Statement::Break => 0,
    }
}

fn count_reads_in_expr(expr: &Expression, name: &str) -> usize {
    match expr {
        Expression::Identifier(id) => usize::from(id == name),
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => count_reads_in_expr(base, name),
        Expression::Call { func, args } => {
            count_reads_in_expr(func, name)
                + args
                    .iter()
                    .map(|a| count_reads_in_expr(a, name))
                    .sum::<usize>()
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            count_reads_in_expr(receiver, name)
                + args
                    .iter()
                    .map(|a| count_reads_in_expr(a, name))
                    .sum::<usize>()
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            count_reads_in_expr(lhs, name) + count_reads_in_expr(rhs, name)
        }
        Expression::Index { base, key } => {
            count_reads_in_expr(base, name) + count_reads_in_expr(key, name)
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            parts.iter().map(|p| count_reads_in_expr(p, name)).sum()
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => fields
            .iter()
            .map(|(_, v)| count_reads_in_expr(v, name))
            .sum(),
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            count_reads_in_expr(condition, name)
                + count_reads_in_expr(then_expr, name)
                + count_reads_in_expr(else_expr, name)
        }
        Expression::Closure { params, body } => {
            if params.iter().any(|p| p == name) {
                0
            } else {
                count_reads(name, &body.statements)
            }
        }
        Expression::Literal(_) | Expression::QualifiedPath { .. } => 0,
    }
}
