use syn::{BinOp, Block, Expr, ExprBinary, Pat, Stmt};

use crate::error::{FrontendError, FrontendResult};

use super::{
    context::LowerContext,
    expressions::{lower_assignment_target, lower_expression},
    functions::lower_function,
    types::{infer_type_from_expression, inferred_source_type, lower_binding},
    util::{item_name, location},
};

pub fn lower_block(
    block: &Block,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::block::Block> {
    let mut statements = Vec::new();
    let last_index = block.stmts.len().saturating_sub(1);

    for (index, statement) in block.stmts.iter().enumerate() {
        let is_tail = index == last_index;
        statements.extend(lower_statement(statement, is_tail, ctx, self_type)?);
    }

    Ok(factorio_ir::block::Block { statements })
}

fn lower_statement(
    statement: &Stmt,
    is_tail: bool,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match statement {
        Stmt::Local(local) => {
            let init = local
                .init
                .as_ref()
                .ok_or_else(|| FrontendError::MissingLetInitializer {
                    location: location(local),
                })?;

            // Handle tuple destructuring: `let (a, b) = (expr_a, expr_b)`
            // Expand into individual `VariableDecl` statements.
            if let Pat::Tuple(pat_tuple) = &local.pat
                && let Expr::Tuple(rhs_tuple) = init.expr.as_ref()
                && pat_tuple.elems.len() == rhs_tuple.elems.len()
            {
                let mut stmts = Vec::new();
                for (pat, rhs) in pat_tuple.elems.iter().zip(rhs_tuple.elems.iter()) {
                    let name = extract_plain_binding(pat).ok_or_else(|| {
                        FrontendError::ExpectedIdentifierPattern {
                            location: location(pat),
                        }
                    })?;
                    let value = lower_expression(rhs, ctx, self_type)?;
                    let ty =
                        infer_type_from_expression(&value).unwrap_or(factorio_ir::r#type::Type::Void);
                    let source_type = inferred_source_type(&ty);
                    stmts.push(factorio_ir::statement::Statement::VariableDecl {
                        name,
                        ty,
                        source_type,
                        value,
                    });
                }
                return Ok(stmts);
            }

            let (name, annotated_type) = lower_binding(&local.pat)?;
            let value = lower_expression(&init.expr, ctx, self_type)?;
            let (ty, source_type) = if let Some((ty, source_type)) = annotated_type {
                (ty, Some(source_type))
            } else {
                let ty =
                    infer_type_from_expression(&value).unwrap_or(factorio_ir::r#type::Type::Void);
                let source_type = inferred_source_type(&ty);
                (ty, source_type)
            };

            Ok(vec![factorio_ir::statement::Statement::VariableDecl {
                name,
                ty,
                source_type,
                value,
            }])
        }
        Stmt::Item(syn::Item::Fn(function)) => {
            Ok(vec![factorio_ir::statement::Statement::FunctionDecl(
                lower_function(function, ctx)?,
            )])
        }
        Stmt::Item(item) => Err(FrontendError::UnsupportedItem {
            item: item_name(item),
            location: location(item),
        }),
        Stmt::Expr(expression, semi) => {
            lower_expression_statement(expression, semi.is_some(), is_tail, ctx, self_type)
        }
        Stmt::Macro(mac) => {
            let expression = Expr::Macro(syn::ExprMacro {
                mac: mac.mac.clone(),
                attrs: mac.attrs.clone(),
            });
            lower_expression_statement(&expression, true, is_tail, ctx, self_type)
        }
    }
}

fn lower_expression_statement(
    expression: &Expr,
    has_semi: bool,
    is_tail: bool,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    // Block-like expressions always expand to potentially multiple statements
    // (e.g. `if let Some(x) = ...` expands to a VariableDecl + Conditional).
    // Handle them uniformly regardless of tail/semicolon position.
    match expression {
        Expr::If(if_expression) => {
            return lower_if_expression(if_expression, ctx, self_type);
        }
        Expr::ForLoop(for_loop) => {
            return lower_for_loop(for_loop, ctx, self_type);
        }
        _ => {}
    }

    if has_semi {
        return Ok(vec![lower_semicolon_expression(
            expression, ctx, self_type,
        )?]);
    }

    if is_tail {
        return Ok(vec![lower_tail_expression(expression, ctx, self_type)?]);
    }

    Err(FrontendError::UnsupportedStatement {
        location: location(expression),
    })
}

fn lower_tail_expression(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::statement::Statement> {
    match expression {
        Expr::If(if_expression) => {
            let stmts = lower_if_expression(if_expression, ctx, self_type)?;
            // If let expansions produce >1 statement; wrap in a no-op Return for the tail slot.
            // In practice the last statement is always a Conditional.
            Ok(stmts
                .into_iter()
                .last()
                .unwrap_or(factorio_ir::statement::Statement::Return(None)))
        }
        Expr::ForLoop(for_loop) => {
            let stmts = lower_for_loop(for_loop, ctx, self_type)?;
            Ok(stmts
                .into_iter()
                .last()
                .unwrap_or(factorio_ir::statement::Statement::Return(None)))
        }
        Expr::Return(return_expression) => Ok(factorio_ir::statement::Statement::Return(
            match return_expression.expr.as_deref() {
                Some(value) => Some(lower_expression(value, ctx, self_type)?),
                None => None,
            },
        )),
        Expr::Continue(_) => Ok(factorio_ir::statement::Statement::Continue),
        _ => Ok(factorio_ir::statement::Statement::Return(Some(
            lower_expression(expression, ctx, self_type)?,
        ))),
    }
}

fn lower_semicolon_expression(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::statement::Statement> {
    match expression {
        Expr::Return(return_expression) => Ok(factorio_ir::statement::Statement::Return(
            match return_expression.expr.as_deref() {
                Some(value) => Some(lower_expression(value, ctx, self_type)?),
                None => None,
            },
        )),
        Expr::Assign(assign) => Ok(lower_assign_statement(assign, ctx, self_type)?),
        Expr::Binary(binary) if is_compound_assign(&binary.op) => {
            Ok(lower_compound_assign_statement(binary, ctx, self_type)?)
        }
        Expr::If(if_expression) => {
            let stmts = lower_if_expression(if_expression, ctx, self_type)?;
            Ok(stmts
                .into_iter()
                .last()
                .unwrap_or(factorio_ir::statement::Statement::Return(None)))
        }
        Expr::Call(_) | Expr::MethodCall(_) | Expr::Macro(_) => Ok(
            factorio_ir::statement::Statement::Expr(lower_expression(expression, ctx, self_type)?),
        ),
        Expr::Continue(_) => Ok(factorio_ir::statement::Statement::Continue),
        Expr::ForLoop(for_loop) => {
            let stmts = lower_for_loop(for_loop, ctx, self_type)?;
            Ok(stmts
                .into_iter()
                .last()
                .unwrap_or(factorio_ir::statement::Statement::Return(None)))
        }
        _ => Err(FrontendError::UnsupportedStatement {
            location: location(expression),
        }),
    }
}

fn lower_for_loop(
    for_loop: &syn::ExprForLoop,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let var = extract_plain_binding(&for_loop.pat).ok_or_else(|| {
        FrontendError::ExpectedIdentifierPattern {
            location: location(&for_loop.pat),
        }
    })?;
    let iter = lower_expression(&for_loop.expr, ctx, self_type)?;
    let body = lower_block_statements(&for_loop.body.stmts, ctx, self_type)?;
    Ok(vec![factorio_ir::statement::Statement::ForIn {
        var,
        iter,
        body,
    }])
}

fn lower_assign_statement(
    assign: &syn::ExprAssign,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::statement::Statement> {
    Ok(factorio_ir::statement::Statement::Assignment {
        target: lower_assignment_target(&assign.left, ctx, self_type)?,
        value: lower_expression(&assign.right, ctx, self_type)?,
    })
}

const fn is_compound_assign(operator: &BinOp) -> bool {
    matches!(
        operator,
        BinOp::AddAssign(_) | BinOp::SubAssign(_) | BinOp::MulAssign(_) | BinOp::DivAssign(_)
    )
}

fn lower_compound_assign_statement(
    binary: &ExprBinary,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::statement::Statement> {
    let operator = compound_assign_operator(&binary.op)?;
    let target = lower_assignment_target(&binary.left, ctx, self_type)?;
    let rhs = lower_expression(&binary.right, ctx, self_type)?;

    Ok(factorio_ir::statement::Statement::Assignment {
        target: target.clone(),
        value: factorio_ir::expression::Expression::BinaryOp {
            lhs: Box::new(target),
            op: operator,
            rhs: Box::new(rhs),
        },
    })
}

fn compound_assign_operator(operator: &BinOp) -> FrontendResult<factorio_ir::operator::Operator> {
    let operator = match operator {
        BinOp::AddAssign(_) => factorio_ir::operator::Operator::Add,
        BinOp::SubAssign(_) => factorio_ir::operator::Operator::Sub,
        BinOp::MulAssign(_) => factorio_ir::operator::Operator::Mul,
        BinOp::DivAssign(_) => factorio_ir::operator::Operator::Div,
        _ => {
            return Err(FrontendError::UnsupportedOperator {
                location: location(operator),
            });
        }
    };

    Ok(operator)
}

fn lower_if_expression(
    if_expression: &syn::ExprIf,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    // Handle `if let Some(x) = expr { ... }`:
    // Lower as `local x = expr` followed by `if x then ... end`.
    if let Expr::Let(let_expr) = if_expression.cond.as_ref()
        && let Some(binding) = extract_some_binding(&let_expr.pat)
    {
            let rhs = lower_expression(&let_expr.expr, ctx, self_type)?;
            let then_block =
                lower_block_statements(&if_expression.then_branch.stmts, ctx, self_type)?;
            let else_block = match &if_expression.else_branch {
                Some((_, else_branch)) => lower_branch_statements(else_branch, ctx, self_type)?,
                None => Vec::new(),
            };
            return Ok(vec![
                factorio_ir::statement::Statement::VariableDecl {
                    name: binding.clone(),
                    ty: factorio_ir::r#type::Type::Void,
                    source_type: None,
                    value: rhs,
                },
                factorio_ir::statement::Statement::Conditional {
                    condition: factorio_ir::expression::Expression::Identifier(binding),
                    then_block,
                    else_block,
                },
            ]);
    }

    let condition = lower_expression(&if_expression.cond, ctx, self_type)?;
    let then_block = lower_block_statements(&if_expression.then_branch.stmts, ctx, self_type)?;
    let else_block = match &if_expression.else_branch {
        Some((_, else_branch)) => lower_branch_statements(else_branch, ctx, self_type)?,
        None => Vec::new(),
    };

    Ok(vec![factorio_ir::statement::Statement::Conditional {
        condition,
        then_block,
        else_block,
    }])
}

/// Extract the inner binding name from `Some(x)` or plain `x` patterns used in `if let`.
/// Returns `None` for unsupported patterns (caller will fall through to a regular condition).
fn extract_some_binding(pat: &Pat) -> Option<String> {
    match pat {
        // `if let Some(x) = ...`
        Pat::TupleStruct(ts) => {
            let last = ts.path.segments.last()?;
            if last.ident != "Some" {
                return None;
            }
            let inner = ts.elems.first()?;
            extract_plain_binding(inner)
        }
        // `if let x = ...` (plain binding without wrapper)
        other => extract_plain_binding(other),
    }
}

fn extract_plain_binding(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(ident) => Some(ident.ident.to_string()),
        Pat::Type(pat_type) => extract_plain_binding(&pat_type.pat),
        _ => None,
    }
}

fn lower_branch_statements(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match expression {
        Expr::Block(block) => lower_block_statements(&block.block.stmts, ctx, self_type),
        _ => Err(FrontendError::UnsupportedStatement {
            location: location(expression),
        }),
    }
}

fn lower_block_statements(
    statements: &[Stmt],
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let mut lowered = Vec::new();
    let last_index = statements.len().saturating_sub(1);

    for (index, statement) in statements.iter().enumerate() {
        let is_tail = index == last_index;
        lowered.extend(lower_statement(statement, is_tail, ctx, self_type)?);
    }

    Ok(lowered)
}
