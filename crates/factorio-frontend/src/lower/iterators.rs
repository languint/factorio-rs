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
    Take(&'a Expr),
}

/// Lower supported iterator chains ending in `.collect()` to a Lua IIFE.
///
/// Returns `None` for ordinary method calls so Option/Result lowering retains
/// ownership of their overlapping adapter names.
#[allow(clippy::too_many_lines)]
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

    let mut take_inits = Vec::new();
    let adapters = adapters
        .into_iter()
        .enumerate()
        .map(|(i, adapter)| match adapter {
            Adapter::Map(closure) => {
                lower_closure(closure, ctx, self_type).map(LoweredAdapter::Map)
            }
            Adapter::Filter(closure) => {
                lower_closure(closure, ctx, self_type).map(LoweredAdapter::Filter)
            }
            Adapter::Take(limit_expr) => {
                let limit = lower_expression(limit_expr, ctx, self_type)?;
                let counter = format!("__take_{}", i + 1);
                take_inits.push(Statement::VariableDecl {
                    name: counter.clone(),
                    ty: Type::Void,
                    source_type: None,
                    value: Expression::Literal(Literal::Int(0)),
                });
                Ok(LoweredAdapter::Take { limit, counter })
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

    let mut iife_body = Vec::with_capacity(take_inits.len() + 3);
    iife_body.push(Statement::VariableDecl {
        name: output.clone(),
        ty: Type::Void,
        source_type: None,
        value: Expression::Call {
            func: Box::new(Expression::QualifiedPath {
                segments: vec!["Vec".to_string(), "new".to_string()],
            }),
            args: vec![],
        },
    });
    iife_body.extend(take_inits);
    iife_body.push(loop_statement);
    iife_body.push(Statement::Return(Some(Expression::Identifier(output))));

    Ok(Some(Expression::Call {
        func: Box::new(Expression::Closure {
            params: vec![],
            body: Block {
                statements: iife_body,
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
    Take { limit: Expression, counter: String },
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
            dispatch: factorio_ir::expression::MethodDispatch::Infer,
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
        LoweredAdapter::Take { limit, counter } => {
            let mut then = vec![Statement::Assignment {
                target: Expression::Identifier(counter.clone()),
                value: Expression::BinaryOp {
                    lhs: Box::new(Expression::Identifier(counter.clone())),
                    op: Operator::Add,
                    rhs: Box::new(Expression::Literal(Literal::Int(1))),
                },
            }];
            then.extend(lower_adapter_body(adapters, index + 1, current, output));
            vec![Statement::Conditional {
                condition: Expression::BinaryOp {
                    lhs: Box::new(Expression::Identifier(counter.clone())),
                    op: Operator::Lt,
                    rhs: Box::new(limit.clone()),
                },
                then_block: then,
                else_block: vec![Statement::Break],
            }]
        }
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
        Expr::MethodCall(call) if call.method == "take" => {
            let limit = exactly_one_arg(call, "take")?;
            let root = peel_chain(&call.receiver, adapters)?;
            adapters.push(Adapter::Take(limit));
            Ok(root)
        }
        Expr::MethodCall(call)
            if matches!(call.method.to_string().as_str(), "iter" | "into_iter")
                && call.args.is_empty() =>
        {
            // `(0..=n).iter()` is redundant but idiomatic; treat as a numeric range
            // instead of lowering the bare Range (which is otherwise unsupported).
            match strip_parens(&call.receiver) {
                Expr::Range(range) => Ok(IteratorRoot::Range(range)),
                _ => Ok(IteratorRoot::Iter(&call.receiver)),
            }
        }
        Expr::MethodCall(call) => Err(unsupported(
            call,
            format!(
                "unsupported iterator adapter `.{}`; supported adapters are `.map(...)`, `.filter(...)`, and `.take(...)` before `.collect()`",
                call.method
            ),
        )),
        other => Err(unsupported(
            other,
            "`.collect()` is only supported on a range or `v.iter()`/`v.into_iter()` map/filter/take chain",
        )),
    }
}

fn exactly_one_closure(call: &ExprMethodCall) -> FrontendResult<&ExprClosure> {
    let arg = exactly_one_arg(call, &call.method.to_string())?;
    match arg {
        Expr::Closure(closure) => Ok(closure),
        _ => Err(unsupported(
            call,
            format!("`.{}(...)` requires a closure argument", call.method),
        )),
    }
}

fn exactly_one_arg<'a>(call: &'a ExprMethodCall, method: &str) -> FrontendResult<&'a Expr> {
    if call.args.len() != 1 {
        return Err(unsupported(
            call,
            format!("`.{method}(...)` requires exactly one argument"),
        ));
    }
    Ok(&call.args[0])
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
