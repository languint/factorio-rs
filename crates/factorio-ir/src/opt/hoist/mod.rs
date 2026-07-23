mod expand;
mod expr;
mod extract;

use crate::{
    block::Block,
    expression::Expression,
    function::Function,
    module::{Module, Symbol},
    statement::Statement,
};

use expand::expand_statement;
use expr::optimize_expression;

pub(super) fn optimize_module(module: &mut Module) {
    optimize_block(&mut module.body);
    for symbol in &mut module.symbols {
        optimize_symbol(symbol);
    }
}

fn optimize_symbol(symbol: &mut Symbol) {
    optimize_statement_inplace(&mut symbol.statement);
}

fn optimize_block(block: &mut Block) {
    block.statements = optimize_statements(std::mem::take(&mut block.statements));
}

fn optimize_statement_inplace(statement: &mut Statement) {
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
            *then_block = optimize_statements(std::mem::take(then_block));
            *else_block = optimize_statements(std::mem::take(else_block));
        }
        Statement::Return(None) | Statement::Continue | Statement::Break => {}
        Statement::ForIn { iter, body, .. } => {
            optimize_expression(iter);
            *body = optimize_statements(std::mem::take(body));
        }
        Statement::ForNumeric {
            start, limit, body, ..
        } => {
            optimize_expression(start);
            optimize_expression(limit);
            *body = optimize_statements(std::mem::take(body));
        }
        Statement::While { condition, body } => {
            optimize_expression(condition);
            *body = optimize_statements(std::mem::take(body));
        }
    }
}

fn optimize_function(function: &mut Function) {
    optimize_block(&mut function.body);
    if let Some(filter) = &mut function.event_filter {
        optimize_expression(filter);
    }
}

fn optimize_statements(statements: Vec<Statement>) -> Vec<Statement> {
    let mut out = Vec::with_capacity(statements.len());
    let mut hoist_counter = 0u32;
    for statement in statements {
        out.extend(expand_statement(statement, &mut hoist_counter));
    }
    out
}

const fn is_simple_assign_target(target: &Expression) -> bool {
    matches!(
        target,
        Expression::Identifier(_)
            | Expression::FieldAccess { .. }
            | Expression::Index { .. }
            | Expression::QualifiedPath { .. }
    )
}
