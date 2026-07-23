use crate::{
    block::Block, expression::Expression, function::Function, literal::Literal, module::Module,
    operator::Operator, statement::Statement, r#type::Type,
};

pub(super) fn optimize_module(module: &mut Module) {
    optimize_block(&mut module.body);
    for symbol in &mut module.symbols {
        optimize_statement(&mut symbol.statement);
    }
}

fn optimize_block(block: &mut Block) {
    let statements = std::mem::take(&mut block.statements);
    block.statements = simplify_statements(&statements);
}

fn optimize_statement(statement: &mut Statement) {
    match statement {
        Statement::FunctionDecl(function) => optimize_function(function),
        Statement::StructDecl(struct_decl) => {
            for method in &mut struct_decl.methods {
                optimize_function(method);
            }
        }
        Statement::EnumDecl(enum_decl) => {
            for method in &mut enum_decl.methods {
                optimize_function(method);
            }
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
            ..
        } => {
            fold_bool_if_expr(condition);
            let then_taken = std::mem::take(then_block);
            *then_block = simplify_statements(&then_taken);
            let else_taken = std::mem::take(else_block);
            *else_block = simplify_statements(&else_taken);
        }
        Statement::ForIn { iter, body, .. } => {
            fold_bool_if_expr(iter);
            let taken = std::mem::take(body);
            *body = simplify_statements(&taken);
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            fold_bool_if_expr(start);
            fold_bool_if_expr(limit);
            let taken = std::mem::take(body);
            *body = simplify_statements(&taken);
        }
        Statement::While { condition, body } => {
            fold_bool_if_expr(condition);
            let taken = std::mem::take(body);
            *body = simplify_statements(&taken);
        }
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => fold_bool_if_expr(value),
        Statement::Assignment { target, value } => {
            fold_bool_if_expr(target);
            fold_bool_if_expr(value);
        }
        Statement::Return(None) | Statement::Continue | Statement::Break => {}
    }
}

fn optimize_function(function: &mut Function) {
    optimize_block(&mut function.body);
}

fn simplify_statements(statements: &[Statement]) -> Vec<Statement> {
    let mut out = Vec::with_capacity(statements.len());
    let mut i = 0;
    while i < statements.len() {
        if let Some((consumed, replacement)) = try_simplify_unwrap_or(&statements[i..]) {
            out.extend(replacement);
            i += consumed;
            continue;
        }
        let mut statement = statements[i].clone();
        optimize_statement(&mut statement);
        if let Some(folded) = try_fold_bool_conditional(&statement) {
            out.push(folded);
        } else {
            out.push(statement);
        }
        i += 1;
    }
    eliminate_copy_temps(out)
}

/// `if c { true } else { false }` -> `c` (and swapped -> `not c`).
fn fold_bool_if_expr(expr: &mut Expression) {
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
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
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
            body.statements = simplify_statements(&statements);
        }
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {}
    }
}

/// `if c then return true else return false end` -> `return c`
/// (and assign / `not` variants).
fn try_fold_bool_conditional(statement: &Statement) -> Option<Statement> {
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

fn single_return_bool(block: &[Statement]) -> Option<bool> {
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

/// `local n = nil; [local tmp = recv;] if tmp ~= nil then n = tmp else n = d end`
/// -> `local n = recv; if n == nil then n = d end`
fn try_simplify_unwrap_or(stmts: &[Statement]) -> Option<(usize, Vec<Statement>)> {
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

fn ne_nil_ident(condition: &Expression) -> Option<String> {
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

/// Fold `local __tmp = <pure|single-use>; ...` away for compiler temps.
fn eliminate_copy_temps(mut stmts: Vec<Statement>) -> Vec<Statement> {
    let mut i = 0;
    while i < stmts.len() {
        let Statement::VariableDecl { name, value, .. } = &stmts[i] else {
            i += 1;
            continue;
        };
        if !is_compiler_temp(name) {
            i += 1;
            continue;
        }

        let name = name.clone();
        let value = value.clone();
        let rest = &stmts[i + 1..];
        if is_written(&name, rest) {
            i += 1;
            continue;
        }

        let reads = count_reads(&name, rest);
        let can_subst = reads == 0 || is_pure(&value) || reads == 1;
        if !can_subst {
            i += 1;
            continue;
        }

        stmts.remove(i);
        if reads > 0 {
            for statement in &mut stmts[i..] {
                replace_ident_in_statement(statement, &name, &value);
            }
        }
        // Restart from i: a later temp may now be eligible, and indices shifted.
    }
    stmts
}

fn is_compiler_temp(name: &str) -> bool {
    name.starts_with("__")
}

fn is_pure(expr: &Expression) -> bool {
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

fn is_written(name: &str, stmts: &[Statement]) -> bool {
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

fn count_reads(name: &str, stmts: &[Statement]) -> usize {
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

fn replace_ident_in_statement(statement: &mut Statement, name: &str, with: &Expression) {
    match statement {
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => replace_ident_in_expr(value, name, with),
        Statement::Assignment { target, value } => {
            replace_ident_in_expr(target, name, with);
            replace_ident_in_expr(value, name, with);
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            replace_ident_in_expr(condition, name, with);
            for s in then_block {
                replace_ident_in_statement(s, name, with);
            }
            for s in else_block {
                replace_ident_in_statement(s, name, with);
            }
        }
        Statement::ForIn { iter, body, .. } => {
            replace_ident_in_expr(iter, name, with);
            for s in body {
                replace_ident_in_statement(s, name, with);
            }
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            replace_ident_in_expr(start, name, with);
            replace_ident_in_expr(limit, name, with);
            for s in body {
                replace_ident_in_statement(s, name, with);
            }
        }
        Statement::While { condition, body } => {
            replace_ident_in_expr(condition, name, with);
            for s in body {
                replace_ident_in_statement(s, name, with);
            }
        }
        Statement::FunctionDecl(function) => {
            if function.params.iter().any(|p| p.name == name) {
                return;
            }
            for s in &mut function.body.statements {
                replace_ident_in_statement(s, name, with);
            }
        }
        Statement::StructDecl(struct_decl) => {
            for method in &mut struct_decl.methods {
                if method.params.iter().any(|p| p.name == name) {
                    continue;
                }
                for s in &mut method.body.statements {
                    replace_ident_in_statement(s, name, with);
                }
            }
        }
        Statement::EnumDecl(enum_decl) => {
            for method in &mut enum_decl.methods {
                if method.params.iter().any(|p| p.name == name) {
                    continue;
                }
                for s in &mut method.body.statements {
                    replace_ident_in_statement(s, name, with);
                }
            }
        }
        Statement::Return(None) | Statement::Continue | Statement::Break => {}
    }
}

fn replace_ident_in_expr(expr: &mut Expression, name: &str, with: &Expression) {
    match expr {
        Expression::Identifier(id) if id == name => *expr = with.clone(),
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => replace_ident_in_expr(base, name, with),
        Expression::Call { func, args } => {
            replace_ident_in_expr(func, name, with);
            for arg in args {
                replace_ident_in_expr(arg, name, with);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            replace_ident_in_expr(receiver, name, with);
            for arg in args {
                replace_ident_in_expr(arg, name, with);
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            replace_ident_in_expr(lhs, name, with);
            replace_ident_in_expr(rhs, name, with);
        }
        Expression::Index { base, key } => {
            replace_ident_in_expr(base, name, with);
            replace_ident_in_expr(key, name, with);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                replace_ident_in_expr(part, name, with);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                replace_ident_in_expr(value, name, with);
            }
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            replace_ident_in_expr(condition, name, with);
            replace_ident_in_expr(then_expr, name, with);
            replace_ident_in_expr(else_expr, name, with);
        }
        Expression::Closure { params, body } => {
            if params.iter().any(|p| p == name) {
                return;
            }
            for statement in &mut body.statements {
                replace_ident_in_statement(statement, name, with);
            }
        }
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {}
    }
}
