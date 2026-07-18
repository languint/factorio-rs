use syn::{Expr, ExprClosure, ExprMethodCall};

use crate::error::{FrontendError, FrontendResult};

use super::{
    context::LowerContext, expressions::lower_expression, functions::lower_closure, util::location,
};

use factorio_ir::{
    block::Block, expression::Expression, literal::Literal, operator::Operator,
    statement::Statement, r#type::Type,
};

enum IteratorRoot<'a> {
    Range(&'a syn::ExprRange),
    Iter(&'a Expr),
}

enum Adapter<'a> {
    Map(&'a ExprClosure),
    Filter(&'a ExprClosure),
}

/// Lower supported iterator chains ending in `.collect()` to a Lua IIFE.
///
/// Returns `None` for ordinary method calls so Option/Result lowering retains
/// ownership of their overlapping adapter names.
pub fn try_lower_iterator_chain(
    call: &ExprMethodCall,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Option<Expression>> {
    if call.method != "collect" {
        return Ok(None);
    }
    if !call.args.is_empty() {
        return Err(unsupported(call, "`.collect()` takes no arguments"));
    }

    let mut adapters = Vec::new();
    let root = peel_chain(&call.receiver, &mut adapters)?;
    let (var, loop_statement) = match root {
        IteratorRoot::Range(range) => {
            let start_expr = range
                .start
                .as_deref()
                .ok_or_else(|| unsupported(range, "open-ended ranges are not supported"))?;
            let end_expr = range
                .end
                .as_deref()
                .ok_or_else(|| unsupported(range, "open-ended ranges are not supported"))?;
            let start = lower_expression(start_expr, ctx, self_type)?;
            let end = lower_expression(end_expr, ctx, self_type)?;
            let limit = match range.limits {
                syn::RangeLimits::Closed(_) => end,
                syn::RangeLimits::HalfOpen(_) => Expression::BinaryOp {
                    lhs: Box::new(end),
                    op: Operator::Sub,
                    rhs: Box::new(Expression::Literal(Literal::Int(1))),
                },
            };
            (
                "__iter_item".to_string(),
                IteratorLoop::Numeric { start, limit },
            )
        }
        IteratorRoot::Iter(receiver) => (
            "__iter_item".to_string(),
            IteratorLoop::Ordered {
                iter: lower_expression(receiver, ctx, self_type)?,
            },
        ),
    };

    let adapters = adapters
        .into_iter()
        .map(|adapter| match adapter {
            Adapter::Map(closure) => {
                lower_closure(closure, ctx, self_type).map(LoweredAdapter::Map)
            }
            Adapter::Filter(closure) => {
                lower_closure(closure, ctx, self_type).map(LoweredAdapter::Filter)
            }
        })
        .collect::<FrontendResult<Vec<_>>>()?;

    let output = "__out".to_string();
    let current = "__iter_value".to_string();
    let mut body = vec![Statement::VariableDecl {
        name: current.clone(),
        ty: Type::Void,
        source_type: None,
        value: Expression::Identifier(var.clone()),
    }];
    body.extend(lower_adapter_body(&adapters, 0, &current, &output));

    let loop_statement = match loop_statement {
        IteratorLoop::Numeric { start, limit } => Statement::ForNumeric {
            var,
            start,
            limit,
            body,
        },
        IteratorLoop::Ordered { iter } => Statement::ForIn {
            var,
            iter,
            body,
            ipairs: true,
        },
    };

    Ok(Some(Expression::Call {
        func: Box::new(Expression::Closure {
            params: vec![],
            body: Block {
                statements: vec![
                    Statement::VariableDecl {
                        name: output.clone(),
                        ty: Type::Void,
                        source_type: None,
                        value: Expression::Call {
                            func: Box::new(Expression::QualifiedPath {
                                segments: vec!["Vec".to_string(), "new".to_string()],
                            }),
                            args: vec![],
                        },
                    },
                    loop_statement,
                    Statement::Return(Some(Expression::Identifier(output))),
                ],
            },
        }),
        args: vec![],
    }))
}

enum IteratorLoop {
    Numeric {
        start: Expression,
        limit: Expression,
    },
    Ordered {
        iter: Expression,
    },
}

enum LoweredAdapter {
    Map(Expression),
    Filter(Expression),
}

fn lower_adapter_body(
    adapters: &[LoweredAdapter],
    index: usize,
    current: &str,
    output: &str,
) -> Vec<Statement> {
    let value = Expression::Identifier(current.to_string());
    let Some(adapter) = adapters.get(index) else {
        return vec![Statement::Expr(Expression::MethodCall {
            receiver: Box::new(Expression::Identifier(output.to_string())),
            method: "push".to_string(),
            args: vec![value],
        })];
    };

    match adapter {
        LoweredAdapter::Map(map) => {
            let mut statements = vec![Statement::Assignment {
                target: Expression::Identifier(current.to_string()),
                value: Expression::Call {
                    func: Box::new(map.clone()),
                    args: vec![value],
                },
            }];
            statements.extend(lower_adapter_body(adapters, index + 1, current, output));
            statements
        }
        LoweredAdapter::Filter(filter) => vec![Statement::Conditional {
            condition: Expression::Call {
                func: Box::new(filter.clone()),
                args: vec![value],
            },
            then_block: lower_adapter_body(adapters, index + 1, current, output),
            else_block: vec![],
        }],
    }
}

fn peel_chain<'a>(
    expression: &'a Expr,
    adapters: &mut Vec<Adapter<'a>>,
) -> FrontendResult<IteratorRoot<'a>> {
    match strip_parens(expression) {
        Expr::Range(range) => Ok(IteratorRoot::Range(range)),
        Expr::MethodCall(call) if matches!(call.method.to_string().as_str(), "map" | "filter") => {
            let closure = exactly_one_closure(call)?;
            let root = peel_chain(&call.receiver, adapters)?;
            if call.method == "map" {
                adapters.push(Adapter::Map(closure));
            } else {
                adapters.push(Adapter::Filter(closure));
            }
            Ok(root)
        }
        Expr::MethodCall(call)
            if matches!(call.method.to_string().as_str(), "iter" | "into_iter")
                && call.args.is_empty() =>
        {
            Ok(IteratorRoot::Iter(&call.receiver))
        }
        Expr::MethodCall(call) => Err(unsupported(
            call,
            format!(
                "unsupported iterator adapter `.{}`; supported adapters are `.map(...)` and `.filter(...)` before `.collect()`",
                call.method
            ),
        )),
        other => Err(unsupported(
            other,
            "`.collect()` is only supported on a range or `v.iter()`/`v.into_iter()` map/filter chain",
        )),
    }
}

fn exactly_one_closure(call: &ExprMethodCall) -> FrontendResult<&ExprClosure> {
    if call.args.len() != 1 {
        return Err(unsupported(
            call,
            format!(
                "`.{}(...)` requires exactly one closure argument",
                call.method
            ),
        ));
    }
    match &call.args[0] {
        Expr::Closure(closure) => Ok(closure),
        _ => Err(unsupported(
            call,
            format!("`.{}(...)` requires a closure argument", call.method),
        )),
    }
}

fn strip_parens(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(paren) => strip_parens(&paren.expr),
        other => other,
    }
}

fn unsupported(item: &impl syn::spanned::Spanned, detail: impl Into<String>) -> FrontendError {
    FrontendError::UnsupportedExpression {
        location: location(item).with_note(detail),
    }
}
