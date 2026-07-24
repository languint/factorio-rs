//! Inline single-use closures at call sites.

use crate::{
    block::Block, expression::Expression, function::Function, module::Module, statement::Statement,
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
    // Children first so nested map closures simplify before outer ifs.
    match expr {
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {
            return;
        }
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

    if let Some(inlined) = try_inline_closure_call(expr) {
        *expr = inlined;
    }
}

fn try_inline_closure_call(expr: &Expression) -> Option<Expression> {
    let Expression::Call { func, args } = expr else {
        return None;
    };
    let Expression::Closure { params, body } = func.as_ref() else {
        return None;
    };
    if params.len() != args.len() {
        return None;
    }
    if !args.iter().all(is_trivial_arg) {
        return None;
    }
    let [Statement::Return(Some(ret))] = body.statements.as_slice() else {
        return None;
    };
    // Trivial args (idents/lits) may be rematerialized when a param is used more
    // than once; that is cheaper than leaving a CLOSURE+CALL in Factorio Lua.

    let mut result = ret.clone();
    for (param, arg) in params.iter().zip(args.iter()) {
        substitute_ident(&mut result, param, arg);
    }
    Some(result)
}

const fn is_trivial_arg(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::Identifier(_) | Expression::Literal(_) | Expression::QualifiedPath { .. }
    )
}

fn substitute_ident(expr: &mut Expression, name: &str, replacement: &Expression) {
    match expr {
        Expression::Identifier(id) if id == name => {
            *expr = replacement.clone();
        }
        Expression::Literal(_) | Expression::Identifier(_) | Expression::QualifiedPath { .. } => {}
        Expression::FieldAccess { base, .. }
        | Expression::Not(base)
        | Expression::Len(base)
        | Expression::FatPointer { data: base, .. } => substitute_ident(base, name, replacement),
        Expression::Call { func, args } => {
            substitute_ident(func, name, replacement);
            for arg in args {
                substitute_ident(arg, name, replacement);
            }
        }
        Expression::MethodCall { receiver, args, .. }
        | Expression::DynMethodCall { receiver, args, .. } => {
            substitute_ident(receiver, name, replacement);
            for arg in args {
                substitute_ident(arg, name, replacement);
            }
        }
        Expression::StructLiteral { fields, .. } | Expression::EnumLiteral { fields, .. } => {
            for (_, value) in fields {
                substitute_ident(value, name, replacement);
            }
        }
        Expression::BinaryOp { lhs, rhs, .. } => {
            substitute_ident(lhs, name, replacement);
            substitute_ident(rhs, name, replacement);
        }
        Expression::FormatConcat { parts } | Expression::Array { elements: parts } => {
            for part in parts {
                substitute_ident(part, name, replacement);
            }
        }
        Expression::Index { base, key } => {
            substitute_ident(base, name, replacement);
            substitute_ident(key, name, replacement);
        }
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            substitute_ident(condition, name, replacement);
            substitute_ident(then_expr, name, replacement);
            substitute_ident(else_expr, name, replacement);
        }
        Expression::Closure { body, params } => {
            // Don't substitute shadowed params.
            if params.iter().any(|p| p == name) {
                return;
            }
            for statement in &mut body.statements {
                substitute_ident_in_statement(statement, name, replacement);
            }
        }
    }
}

fn substitute_ident_in_statement(statement: &mut Statement, name: &str, replacement: &Expression) {
    match statement {
        Statement::VariableDecl { value, .. }
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => {
            substitute_ident(value, name, replacement);
        }
        Statement::Assignment { target, value } => {
            substitute_ident(target, name, replacement);
            substitute_ident(value, name, replacement);
        }
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            substitute_ident(condition, name, replacement);
            for stmt in then_block {
                substitute_ident_in_statement(stmt, name, replacement);
            }
            for stmt in else_block {
                substitute_ident_in_statement(stmt, name, replacement);
            }
        }
        Statement::ForIn { iter, body, .. } => {
            substitute_ident(iter, name, replacement);
            for stmt in body {
                substitute_ident_in_statement(stmt, name, replacement);
            }
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            substitute_ident(start, name, replacement);
            substitute_ident(limit, name, replacement);
            for stmt in body {
                substitute_ident_in_statement(stmt, name, replacement);
            }
        }
        Statement::While { condition, body } => {
            substitute_ident(condition, name, replacement);
            for stmt in body {
                substitute_ident_in_statement(stmt, name, replacement);
            }
        }
        Statement::Return(None)
        | Statement::Continue
        | Statement::Break
        | Statement::FunctionDecl(_)
        | Statement::StructDecl(_)
        | Statement::EnumDecl(_)
        | Statement::RawLua { .. } => {}
    }
}
