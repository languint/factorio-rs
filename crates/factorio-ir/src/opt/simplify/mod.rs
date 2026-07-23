mod bool_fold;
mod cleanup;
mod temps;
mod unwrap_or;

use crate::{block::Block, function::Function, module::Module, statement::Statement};

use bool_fold::{fold_bool_cmp_condition, fold_bool_if_expr, try_fold_bool_conditional};
use cleanup::{collapse_nil_inits, drop_redundant_nil_else, simplify_identity_binds};
use temps::eliminate_copy_temps;
use unwrap_or::{try_simplify_result_unwrap_or, try_simplify_unwrap_or};

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
            fold_bool_cmp_condition(condition);
            let then_taken = std::mem::take(then_block);
            *then_block = simplify_statements(&then_taken);
            let else_taken = std::mem::take(else_block);
            *else_block = simplify_statements(&else_taken);
            simplify_identity_binds(then_block);
            simplify_identity_binds(else_block);
            drop_redundant_nil_else(condition, else_block);
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
        | Statement::Expr(value) => {
            fold_bool_if_expr(value);
        }
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
        if let Some((consumed, replacement)) = try_simplify_result_unwrap_or(&statements[i..]) {
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
    let out = eliminate_copy_temps(out);
    let out = collapse_nil_inits(&out);
    eliminate_copy_temps(out)
}
