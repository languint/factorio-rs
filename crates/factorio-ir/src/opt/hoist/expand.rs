use crate::{
    block::Block, expression::Expression, literal::Literal, statement::Statement, r#type::Type,
};

use super::{
    extract::extract_mid_expr_hoists, is_simple_assign_target, optimize_statement_inplace,
    optimize_statements,
};

pub(super) fn expand_statement(
    mut statement: Statement,
    hoist_counter: &mut u32,
) -> Vec<Statement> {
    optimize_statement_inplace(&mut statement);

    let mut prefix = Vec::new();
    extract_mid_expr_hoists(&mut statement, &mut prefix, hoist_counter);

    let expanded = match statement {
        Statement::VariableDecl {
            name,
            ty,
            source_type,
            value,
        } => expand_value_binding(
            ValueSink::Local {
                name,
                ty,
                source_type,
            },
            value,
        ),
        Statement::Assignment { target, value } if is_simple_assign_target(&target) => {
            expand_value_binding(ValueSink::Assign { target }, value)
        }
        Statement::Return(Some(value)) => expand_value_binding(ValueSink::Return, value),
        other => vec![other],
    };

    prefix.extend(expanded);
    prefix
}

pub(super) enum ValueSink {
    Local {
        name: String,
        ty: Type,
        source_type: Option<String>,
    },
    Assign {
        target: Expression,
    },
    Return,
}

pub(super) fn expand_value_binding(sink: ValueSink, value: Expression) -> Vec<Statement> {
    match value {
        Expression::If {
            condition,
            then_expr,
            else_expr,
        } => expand_if(sink, *condition, *then_expr, *else_expr),
        Expression::Call { func, args } if args.is_empty() => match *func {
            Expression::Closure { params, body } if params.is_empty() => expand_iife(sink, body),
            other => finish_sink(
                sink,
                Expression::Call {
                    func: Box::new(other),
                    args,
                },
            ),
        },
        value => finish_sink(sink, value),
    }
}

fn finish_sink(sink: ValueSink, value: Expression) -> Vec<Statement> {
    match sink {
        ValueSink::Local {
            name,
            ty,
            source_type,
        } => vec![Statement::VariableDecl {
            name,
            ty,
            source_type,
            value,
        }],
        ValueSink::Assign { target } => vec![Statement::Assignment { target, value }],
        ValueSink::Return => vec![Statement::Return(Some(value))],
    }
}

fn expand_if(
    sink: ValueSink,
    condition: Expression,
    then_expr: Expression,
    else_expr: Expression,
) -> Vec<Statement> {
    match sink {
        ValueSink::Local {
            name,
            ty,
            source_type,
        } => {
            let then_block = optimize_statements(vec![Statement::Assignment {
                target: Expression::Identifier(name.clone()),
                value: then_expr,
            }]);
            let else_block = optimize_statements(vec![Statement::Assignment {
                target: Expression::Identifier(name.clone()),
                value: else_expr,
            }]);
            vec![
                Statement::VariableDecl {
                    name,
                    ty,
                    source_type,
                    value: Expression::Literal(Literal::Nil),
                },
                Statement::Conditional {
                    condition,
                    then_block,
                    else_block,
                },
            ]
        }
        ValueSink::Assign { target } => {
            let then_block = optimize_statements(vec![Statement::Assignment {
                target: target.clone(),
                value: then_expr,
            }]);
            let else_block = optimize_statements(vec![Statement::Assignment {
                target,
                value: else_expr,
            }]);
            vec![Statement::Conditional {
                condition,
                then_block,
                else_block,
            }]
        }
        ValueSink::Return => {
            let then_block = optimize_statements(vec![Statement::Return(Some(then_expr))]);
            let else_block = optimize_statements(vec![Statement::Return(Some(else_expr))]);
            vec![Statement::Conditional {
                condition,
                then_block,
                else_block,
            }]
        }
    }
}

fn expand_iife(sink: ValueSink, body: Block) -> Vec<Statement> {
    match sink {
        ValueSink::Local {
            name,
            ty,
            source_type,
        } => {
            let mut out = vec![Statement::VariableDecl {
                name: name.clone(),
                ty,
                source_type,
                value: Expression::Literal(Literal::Nil),
            }];
            out.extend(optimize_statements(remap_returns_to_assign(
                body.statements,
                &Expression::Identifier(name),
            )));
            out
        }
        ValueSink::Assign { target } => {
            optimize_statements(remap_returns_to_assign(body.statements, &target))
        }
        ValueSink::Return => optimize_statements(body.statements),
    }
}

fn remap_returns_to_assign(statements: Vec<Statement>, target: &Expression) -> Vec<Statement> {
    statements
        .into_iter()
        .map(|statement| remap_return_statement(statement, target))
        .collect()
}

fn remap_return_statement(statement: Statement, target: &Expression) -> Statement {
    match statement {
        Statement::Return(value) => Statement::Assignment {
            target: target.clone(),
            value: value.unwrap_or(Expression::Literal(Literal::Nil)),
        },
        Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => Statement::Conditional {
            condition,
            then_block: then_block
                .into_iter()
                .map(|s| remap_return_statement(s, target))
                .collect(),
            else_block: else_block
                .into_iter()
                .map(|s| remap_return_statement(s, target))
                .collect(),
        },
        Statement::ForIn {
            var,
            iter,
            body,
            ipairs,
        } => Statement::ForIn {
            var,
            iter,
            body: body
                .into_iter()
                .map(|s| remap_return_statement(s, target))
                .collect(),
            ipairs,
        },
        Statement::ForNumeric {
            var,
            start,
            limit,
            body,
        } => Statement::ForNumeric {
            var,
            start,
            limit,
            body: body
                .into_iter()
                .map(|s| remap_return_statement(s, target))
                .collect(),
        },
        Statement::While { condition, body } => Statement::While {
            condition,
            body: body
                .into_iter()
                .map(|s| remap_return_statement(s, target))
                .collect(),
        },
        // Nested function values keep their own returns.
        other => other,
    }
}
