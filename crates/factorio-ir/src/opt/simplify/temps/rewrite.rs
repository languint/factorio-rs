use crate::{expression::Expression, statement::Statement};

pub(super) fn replace_ident_in_statement(statement: &mut Statement, name: &str, with: &Expression) {
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
        Statement::Return(None)
        | Statement::Continue
        | Statement::Break
        | Statement::RawLua { .. } => {}
    }
}

pub(super) fn replace_ident_in_expr(expr: &mut Expression, name: &str, with: &Expression) {
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
