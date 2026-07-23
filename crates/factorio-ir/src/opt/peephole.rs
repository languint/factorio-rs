use crate::{
    block::Block, expression::Expression, function::Function, module::Module, operator::Operator,
    statement::Statement,
};

pub(super) fn optimize_module(module: &mut Module) {
    let mut counter = 0usize;
    optimize_block(&mut module.body, &mut counter);
    for symbol in &mut module.symbols {
        optimize_statement(&mut symbol.statement, &mut counter);
    }
}

fn optimize_statement(statement: &mut Statement, counter: &mut usize) {
    match statement {
        Statement::FunctionDecl(function) => optimize_function(function, counter),
        Statement::StructDecl(struct_decl) => {
            for method in &mut struct_decl.methods {
                optimize_function(method, counter);
            }
        }
        Statement::EnumDecl(enum_decl) => {
            for method in &mut enum_decl.methods {
                optimize_function(method, counter);
            }
        }
        Statement::Conditional {
            then_block,
            else_block,
            ..
        } => {
            optimize_stmt_list(then_block, counter);
            optimize_stmt_list(else_block, counter);
        }
        Statement::ForIn { body, .. }
        | Statement::ForNumeric { body, .. }
        | Statement::While { body, .. } => optimize_stmt_list(body, counter),
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value)
        | Statement::Assignment { value, .. } => {
            if let Expression::Closure { body, .. } = value {
                optimize_block(body, counter);
            }
        }
        Statement::Return(None) | Statement::Continue | Statement::Break => {}
    }
}

fn optimize_function(function: &mut Function, counter: &mut usize) {
    optimize_block(&mut function.body, counter);
}

fn optimize_block(block: &mut Block, counter: &mut usize) {
    optimize_stmt_list(&mut block.statements, counter);
}

fn optimize_stmt_list(stmts: &mut Vec<Statement>, counter: &mut usize) {
    for statement in stmts.iter_mut() {
        optimize_statement(statement, counter);
    }
    while fold_one_repeat(stmts, counter) {}
}

fn fold_one_repeat(stmts: &mut Vec<Statement>, counter: &mut usize) -> bool {
    let mut counts: Vec<(Expression, usize)> = Vec::new();
    for statement in stmts.iter() {
        for_each_expr_in_statement(statement, &mut |expr| {
            if is_arith_candidate(expr) {
                bump_count(&mut counts, expr);
            }
        });
    }
    counts.retain(|(_, n)| *n >= 2);
    counts.sort_by(|(a, _), (b, _)| expr_weight(b).cmp(&expr_weight(a)));

    for (expr, _) in counts {
        if try_fold_expr(stmts, &expr, counter) {
            return true;
        }
    }
    false
}

fn bump_count(counts: &mut Vec<(Expression, usize)>, expr: &Expression) {
    if let Some((_, n)) = counts.iter_mut().find(|(e, _)| e == expr) {
        *n += 1;
    } else {
        counts.push((expr.clone(), 1));
    }
}

fn expr_weight(expr: &Expression) -> usize {
    match expr {
        Expression::BinaryOp { lhs, rhs, .. } => 1 + expr_weight(lhs) + expr_weight(rhs),
        _ => 1,
    }
}

fn is_arith_candidate(expr: &Expression) -> bool {
    let Expression::BinaryOp { lhs, op, rhs } = expr else {
        return false;
    };
    if !matches!(
        op,
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div | Operator::Mod
    ) {
        return false;
    }
    is_atom(lhs)
        && is_atom(rhs)
        && (matches!(lhs.as_ref(), Expression::Identifier(_))
            || matches!(rhs.as_ref(), Expression::Identifier(_)))
}

fn is_atom(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::Identifier(_) | Expression::Literal(_) | Expression::QualifiedPath { .. }
    )
}

/// `x` in `x + 1` / `1 + x` (commutative) / `x - 1`, etc.
fn mutate_local_name(expr: &Expression) -> Option<&str> {
    let Expression::BinaryOp { lhs, op, rhs } = expr else {
        return None;
    };
    match (lhs.as_ref(), rhs.as_ref(), op) {
        (Expression::Identifier(name), Expression::Literal(_), _) => Some(name.as_str()),
        (Expression::Literal(_), Expression::Identifier(name), Operator::Add | Operator::Mul) => {
            Some(name.as_str())
        }
        _ => None,
    }
}

fn try_fold_expr(stmts: &mut Vec<Statement>, expr: &Expression, counter: &mut usize) -> bool {
    let Some((first, last)) = occurrence_span(stmts, expr) else {
        return false;
    };
    if let Some(local) = mutate_local_name(expr)
        && !is_written_in_range(stmts, local, first, last)
        && bare_ident_reads_in_range(stmts, local, expr, first, last) == 0
    {
        let assign = Statement::Assignment {
            target: Expression::Identifier(local.to_string()),
            value: expr.clone(),
        };
        stmts.insert(first, assign);
        let last = last + 1;
        let replacement = Expression::Identifier(local.to_string());
        for statement in &mut stmts[first + 1..=last] {
            replace_expr_in_statement(statement, expr, &replacement);
        }
        return true;
    }

    if free_idents_written_in_range(stmts, expr, first, last) {
        return false;
    }
    *counter += 1;
    let tmp = format!("__a_{counter}");
    let decl = Statement::VariableDecl {
        name: tmp.clone(),
        ty: crate::r#type::Type::Void,
        source_type: None,
        value: expr.clone(),
    };
    stmts.insert(first, decl);
    let last = last + 1;
    let replacement = Expression::Identifier(tmp);
    for statement in &mut stmts[first + 1..=last] {
        replace_expr_in_statement(statement, expr, &replacement);
    }
    true
}

fn occurrence_span(stmts: &[Statement], expr: &Expression) -> Option<(usize, usize)> {
    let mut first = None;
    let mut last = None;
    for (i, statement) in stmts.iter().enumerate() {
        let mut hit = false;
        for_each_expr_in_statement(statement, &mut |e| {
            if e == expr {
                hit = true;
            }
        });
        if hit {
            first.get_or_insert(i);
            last = Some(i);
        }
    }
    Some((first?, last?))
}

fn is_written_in_range(stmts: &[Statement], name: &str, first: usize, last: usize) -> bool {
    stmts[first..=last]
        .iter()
        .any(|s| statement_writes(s, name))
}

fn statement_writes(statement: &Statement, name: &str) -> bool {
    match statement {
        Statement::Assignment {
            target: Expression::Identifier(n),
            ..
        } => n == name,
        Statement::VariableDecl { name: n, .. } => n == name,
        Statement::Conditional {
            then_block,
            else_block,
            ..
        } => {
            then_block.iter().any(|s| statement_writes(s, name))
                || else_block.iter().any(|s| statement_writes(s, name))
        }
        Statement::ForIn { var, body, .. } | Statement::ForNumeric { var, body, .. } => {
            var == name || body.iter().any(|s| statement_writes(s, name))
        }
        Statement::While { body, .. } => body.iter().any(|s| statement_writes(s, name)),
        _ => false,
    }
}

fn bare_ident_reads_in_range(
    stmts: &[Statement],
    name: &str,
    skip_expr: &Expression,
    first: usize,
    last: usize,
) -> usize {
    stmts[first..=last]
        .iter()
        .map(|s| count_bare_ident_in_statement(s, name, skip_expr))
        .sum()
}

fn count_bare_ident_in_statement(statement: &Statement, name: &str, skip: &Expression) -> usize {
    match statement {
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => count_bare_ident_in_expr(value, name, skip),
        Statement::Assignment { target, value } => {
            count_bare_ident_in_expr(target, name, skip)
                + count_bare_ident_in_expr(value, name, skip)
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            count_bare_ident_in_expr(condition, name, skip)
                + then_block
                    .iter()
                    .map(|s| count_bare_ident_in_statement(s, name, skip))
                    .sum::<usize>()
                + else_block
                    .iter()
                    .map(|s| count_bare_ident_in_statement(s, name, skip))
                    .sum::<usize>()
        }
        Statement::ForIn { iter, body, .. } => {
            count_bare_ident_in_expr(iter, name, skip)
                + body
                    .iter()
                    .map(|s| count_bare_ident_in_statement(s, name, skip))
                    .sum::<usize>()
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            count_bare_ident_in_expr(start, name, skip)
                + count_bare_ident_in_expr(limit, name, skip)
                + body
                    .iter()
                    .map(|s| count_bare_ident_in_statement(s, name, skip))
                    .sum::<usize>()
        }
        Statement::While { condition, body } => {
            count_bare_ident_in_expr(condition, name, skip)
                + body
                    .iter()
                    .map(|s| count_bare_ident_in_statement(s, name, skip))
                    .sum::<usize>()
        }
        _ => 0,
    }
}

fn count_bare_ident_in_expr(expr: &Expression, name: &str, skip: &Expression) -> usize {
    if expr == skip {
        return 0;
    }
    match expr {
        Expression::Identifier(id) => usize::from(id == name),
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => count_bare_ident_in_expr(base, name, skip),
        Expression::Call { func, args } => {
            count_bare_ident_in_expr(func, name, skip)
                + args
                    .iter()
                    .map(|a| count_bare_ident_in_expr(a, name, skip))
                    .sum::<usize>()
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            count_bare_ident_in_expr(receiver, name, skip)
                + args
                    .iter()
                    .map(|a| count_bare_ident_in_expr(a, name, skip))
                    .sum::<usize>()
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            count_bare_ident_in_expr(lhs, name, skip) + count_bare_ident_in_expr(rhs, name, skip)
        }
        Expression::Index { base, key } => {
            count_bare_ident_in_expr(base, name, skip) + count_bare_ident_in_expr(key, name, skip)
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => parts
            .iter()
            .map(|p| count_bare_ident_in_expr(p, name, skip))
            .sum(),
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => fields
            .iter()
            .map(|(_, v)| count_bare_ident_in_expr(v, name, skip))
            .sum(),
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            count_bare_ident_in_expr(condition, name, skip)
                + count_bare_ident_in_expr(then_expr, name, skip)
                + count_bare_ident_in_expr(else_expr, name, skip)
        }
        Expression::Closure { .. } | Expression::Literal(_) | Expression::QualifiedPath { .. } => 0,
    }
}

fn free_idents_written_in_range(
    stmts: &[Statement],
    expr: &Expression,
    first: usize,
    last: usize,
) -> bool {
    let mut idents = Vec::new();
    collect_idents(expr, &mut idents);
    idents
        .iter()
        .any(|name| is_written_in_range(stmts, name, first, last))
}

fn collect_idents(expr: &Expression, out: &mut Vec<String>) {
    match expr {
        Expression::Identifier(name) => {
            if !out.iter().any(|n| n == name) {
                out.push(name.clone());
            }
        }
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => collect_idents(base, out),
        Expression::Call { func, args } => {
            collect_idents(func, out);
            for arg in args {
                collect_idents(arg, out);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            collect_idents(receiver, out);
            for arg in args {
                collect_idents(arg, out);
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            collect_idents(lhs, out);
            collect_idents(rhs, out);
        }
        Expression::Index { base, key } => {
            collect_idents(base, out);
            collect_idents(key, out);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                collect_idents(part, out);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_idents(value, out);
            }
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_idents(condition, out);
            collect_idents(then_expr, out);
            collect_idents(else_expr, out);
        }
        Expression::Literal(_) | Expression::QualifiedPath { .. } | Expression::Closure { .. } => {}
    }
}

fn for_each_expr_in_statement(statement: &Statement, f: &mut impl FnMut(&Expression)) {
    match statement {
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => for_each_expr(value, f),
        Statement::Assignment { target, value } => {
            for_each_expr(target, f);
            for_each_expr(value, f);
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            for_each_expr(condition, f);
            for s in then_block {
                for_each_expr_in_statement(s, f);
            }
            for s in else_block {
                for_each_expr_in_statement(s, f);
            }
        }
        Statement::ForIn { iter, body, .. } => {
            for_each_expr(iter, f);
            for s in body {
                for_each_expr_in_statement(s, f);
            }
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            for_each_expr(start, f);
            for_each_expr(limit, f);
            for s in body {
                for_each_expr_in_statement(s, f);
            }
        }
        Statement::While { condition, body } => {
            for_each_expr(condition, f);
            for s in body {
                for_each_expr_in_statement(s, f);
            }
        }
        Statement::FunctionDecl(function) => {
            for s in &function.body.statements {
                for_each_expr_in_statement(s, f);
            }
        }
        Statement::StructDecl(struct_decl) => {
            for method in &struct_decl.methods {
                for s in &method.body.statements {
                    for_each_expr_in_statement(s, f);
                }
            }
        }
        Statement::EnumDecl(enum_decl) => {
            for method in &enum_decl.methods {
                for s in &method.body.statements {
                    for_each_expr_in_statement(s, f);
                }
            }
        }
        Statement::Return(None) | Statement::Continue | Statement::Break => {}
    }
}

fn for_each_expr(expr: &Expression, f: &mut impl FnMut(&Expression)) {
    f(expr);
    match expr {
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {}
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => for_each_expr(base, f),
        Expression::Call { func, args } => {
            for_each_expr(func, f);
            for arg in args {
                for_each_expr(arg, f);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            for_each_expr(receiver, f);
            for arg in args {
                for_each_expr(arg, f);
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            for_each_expr(lhs, f);
            for_each_expr(rhs, f);
        }
        Expression::Index { base, key } => {
            for_each_expr(base, f);
            for_each_expr(key, f);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                for_each_expr(part, f);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                for_each_expr(value, f);
            }
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            for_each_expr(condition, f);
            for_each_expr(then_expr, f);
            for_each_expr(else_expr, f);
        }
        Expression::Closure { body, .. } => {
            for statement in &body.statements {
                for_each_expr_in_statement(statement, f);
            }
        }
    }
}

fn replace_expr_in_statement(statement: &mut Statement, from: &Expression, to: &Expression) {
    match statement {
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => replace_expr(value, from, to),
        Statement::Assignment { target, value } => {
            replace_expr(target, from, to);
            replace_expr(value, from, to);
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            replace_expr(condition, from, to);
            for s in then_block {
                replace_expr_in_statement(s, from, to);
            }
            for s in else_block {
                replace_expr_in_statement(s, from, to);
            }
        }
        Statement::ForIn { iter, body, .. } => {
            replace_expr(iter, from, to);
            for s in body {
                replace_expr_in_statement(s, from, to);
            }
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            replace_expr(start, from, to);
            replace_expr(limit, from, to);
            for s in body {
                replace_expr_in_statement(s, from, to);
            }
        }
        Statement::While { condition, body } => {
            replace_expr(condition, from, to);
            for s in body {
                replace_expr_in_statement(s, from, to);
            }
        }
        Statement::FunctionDecl(function) => {
            for s in &mut function.body.statements {
                replace_expr_in_statement(s, from, to);
            }
        }
        Statement::StructDecl(struct_decl) => {
            for method in &mut struct_decl.methods {
                for s in &mut method.body.statements {
                    replace_expr_in_statement(s, from, to);
                }
            }
        }
        Statement::EnumDecl(enum_decl) => {
            for method in &mut enum_decl.methods {
                for s in &mut method.body.statements {
                    replace_expr_in_statement(s, from, to);
                }
            }
        }
        Statement::Return(None) | Statement::Continue | Statement::Break => {}
    }
}

fn replace_expr(expr: &mut Expression, from: &Expression, to: &Expression) {
    if *expr == *from {
        *expr = to.clone();
        return;
    }
    match expr {
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {}
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => replace_expr(base, from, to),
        Expression::Call { func, args } => {
            replace_expr(func, from, to);
            for arg in args {
                replace_expr(arg, from, to);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            replace_expr(receiver, from, to);
            for arg in args {
                replace_expr(arg, from, to);
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            replace_expr(lhs, from, to);
            replace_expr(rhs, from, to);
        }
        Expression::Index { base, key } => {
            replace_expr(base, from, to);
            replace_expr(key, from, to);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                replace_expr(part, from, to);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                replace_expr(value, from, to);
            }
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            replace_expr(condition, from, to);
            replace_expr(then_expr, from, to);
            replace_expr(else_expr, from, to);
        }
        Expression::Closure { body, .. } => {
            for statement in &mut body.statements {
                replace_expr_in_statement(statement, from, to);
            }
        }
    }
}
