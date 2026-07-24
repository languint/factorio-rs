use crate::{
    block::Block, expression::Expression, function::Function, literal::Literal, module::Module,
    statement::Statement,
};

pub(super) fn optimize_module(module: &mut Module) {
    optimize_block(&mut module.body);
    for symbol in &mut module.symbols {
        optimize_statement(&mut symbol.statement);
    }
}

fn optimize_block(block: &mut Block) {
    for statement in &mut block.statements {
        optimize_statement(statement);
    }
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
            for (_, value) in &mut enum_decl.constants {
                optimize_expression(value);
            }
        }
        Statement::VariableDecl { value, .. }
        | Statement::Return(Some(value))
        | Statement::Expr(value) => optimize_expression(value),
        Statement::Assignment { target, value } => {
            optimize_expression(target);
            optimize_expression(value);
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            optimize_expression(condition);
            for stmt in then_block {
                optimize_statement(stmt);
            }
            for stmt in else_block {
                optimize_statement(stmt);
            }
        }
        Statement::Return(None)
        | Statement::Continue
        | Statement::Break
        | Statement::RawLua { .. } => {}
        Statement::ForIn { iter, body, .. } => {
            optimize_expression(iter);
            for stmt in body {
                optimize_statement(stmt);
            }
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            optimize_expression(start);
            optimize_expression(limit);
            for stmt in body {
                optimize_statement(stmt);
            }
        }
        Statement::While { condition, body } => {
            optimize_expression(condition);
            for stmt in body {
                optimize_statement(stmt);
            }
        }
    }
}

fn optimize_function(function: &mut Function) {
    optimize_block(&mut function.body);
    if let Some(filter) = &mut function.event_filter {
        optimize_expression(filter);
    }
}

fn optimize_expression(expr: &mut Expression) {
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
        Expression::Array { elements } => {
            for element in elements {
                optimize_expression(element);
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
        Expression::FormatConcat { parts } => {
            for part in parts.iter_mut() {
                optimize_expression(part);
            }
            *parts = flatten_and_merge(std::mem::take(parts));
            if parts.len() == 1 {
                *expr = parts
                    .pop()
                    .unwrap_or(Expression::Literal(Literal::String(String::new())));
            }
        }
    }
}

fn flatten_and_merge(parts: Vec<Expression>) -> Vec<Expression> {
    let mut flat = Vec::with_capacity(parts.len());
    for part in parts {
        match part {
            Expression::FormatConcat { parts: nested } => flat.extend(nested),
            other => flat.push(other),
        }
    }

    let mut merged = Vec::with_capacity(flat.len());
    for part in flat {
        // Drop empty string operands (`"" .. x` -> `x`).
        if matches!(&part, Expression::Literal(Literal::String(s)) if s.is_empty()) {
            continue;
        }
        match (merged.last_mut(), part) {
            (
                Some(Expression::Literal(Literal::String(prev))),
                Expression::Literal(Literal::String(next)),
            ) => {
                prev.push_str(&next);
            }
            (_, part) => merged.push(part),
        }
    }
    if merged.is_empty() {
        merged.push(Expression::Literal(Literal::String(String::new())));
    }
    merged
}
