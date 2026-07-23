use syn::{Arm, BinOp, Block, Expr, ExprBinary, ExprMatch, Lit, Pat, Stmt};

use crate::error::{FrontendError, FrontendResult};

use super::{
    assert_macros::{is_assert_macro, lower_assert_macro_statements},
    context::LowerContext,
    expressions::{lower_assignment_target, lower_expression},
    functions::lower_function,
    print::infer_debug_type_key,
    types::{
        infer_type_from_expression, inferred_source_type, is_option_type, lower_binding,
        register_type_alias, rust_type_key,
    },
    util::{item_name, location},
};

/// Lower an expression and collect any `?` early-return hoists it emitted.
fn lower_expr(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<(
    Vec<factorio_ir::statement::Statement>,
    factorio_ir::expression::Expression,
)> {
    let mark = ctx.try_hoist_mark();
    let value = lower_expression(expression, ctx, self_type)?;
    Ok((ctx.take_try_hoists_from(mark), value))
}

fn maybe_wrap_return_into(
    value: factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> factorio_ir::expression::Expression {
    if !ctx.return_into {
        return value;
    }
    factorio_ir::expression::Expression::MethodCall {
        receiver: Box::new(value),
        method: "into".to_string(),
        args: Vec::new(),
        dispatch: factorio_ir::expression::MethodDispatch::Infer,
    }
}

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
                    let (mut hoists, value) = lower_expr(rhs, ctx, self_type)?;
                    let ty = infer_type_from_expression(&value)
                        .unwrap_or(factorio_ir::r#type::Type::Void);
                    let source_type = inferred_source_type(&ty);
                    if let Some(key) = infer_debug_type_key(&value, ctx) {
                        ctx.bind_type(name.clone(), key);
                    }
                    hoists.push(factorio_ir::statement::Statement::VariableDecl {
                        name,
                        ty,
                        source_type,
                        value,
                    });
                    stmts.extend(hoists);
                }
                return Ok(stmts);
            }

            let (name, annotated_type) =
                lower_binding(&local.pat, &ctx.type_aliases, &ctx.assoc_bindings)?;
            let (mut hoists, value) = lower_expr(&init.expr, ctx, self_type)?;
            let (ty, source_type) = if let Some((ty, source_type)) = annotated_type {
                (ty, Some(source_type))
            } else {
                let ty =
                    infer_type_from_expression(&value).unwrap_or(factorio_ir::r#type::Type::Void);
                let source_type = inferred_source_type(&ty);
                (ty, source_type)
            };
            bind_let_local_type(&name, &local.pat, &value, &init.expr, ctx);

            hoists.push(factorio_ir::statement::Statement::VariableDecl {
                name,
                ty,
                source_type,
                value,
            });
            Ok(hoists)
        }
        Stmt::Item(syn::Item::Fn(function)) => {
            Ok(vec![factorio_ir::statement::Statement::FunctionDecl(
                lower_function(function, ctx)?,
            )])
        }
        Stmt::Item(syn::Item::Type(item_type)) => {
            register_type_alias(item_type, &mut ctx.type_aliases)?;
            Ok(vec![])
        }
        Stmt::Item(item) => Err(FrontendError::UnsupportedItem {
            item: item_name(item),
            location: location(item),
        }),
        Stmt::Expr(expression, semi) => {
            lower_expression_statement(expression, semi.is_some(), is_tail, ctx, self_type)
        }
        Stmt::Macro(mac) => {
            if is_assert_macro(&mac.mac) {
                let expression = syn::ExprMacro {
                    mac: mac.mac.clone(),
                    attrs: mac.attrs.clone(),
                };
                return lower_assert_macro_statements(&expression, ctx, self_type);
            }
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
        Expr::While(while_expr) => {
            return lower_while_loop(while_expr, ctx, self_type);
        }
        Expr::Loop(loop_expr) => {
            return lower_infinite_loop(loop_expr, ctx, self_type);
        }
        Expr::Match(match_expr) => {
            // Tail value-producing `match` expands to statement arms that `return`.
            if is_tail && !has_semi {
                return lower_match_value_statements(match_expr, ctx, self_type);
            }
            return lower_match_statements(match_expr, ctx, self_type);
        }
        // rustc-expanded `println!` becomes `{ ::std::io::_print(...); };`
        // rustc-expanded `tracing::info!` becomes a block with inner `use` + callsite.
        Expr::Block(block) => {
            if let Some(result) = super::print::try_lower_expanded_tracing_event_block(
                &block.block.stmts,
                ctx,
                self_type,
            ) {
                return result;
            }
            return lower_block_statements(&block.block.stmts, ctx, self_type);
        }
        _ => {}
    }

    if has_semi {
        return lower_semicolon_statements(expression, ctx, self_type);
    }

    if is_tail {
        return lower_tail_statements(expression, ctx, self_type);
    }

    Err(FrontendError::UnsupportedStatement {
        location: location(expression),
    })
}

fn lower_tail_statements(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match expression {
        Expr::If(if_expression) => lower_if_expression(if_expression, ctx, self_type),
        Expr::ForLoop(for_loop) => lower_for_loop(for_loop, ctx, self_type),
        Expr::While(while_expr) => lower_while_loop(while_expr, ctx, self_type),
        Expr::Loop(loop_expr) => lower_infinite_loop(loop_expr, ctx, self_type),
        Expr::Match(match_expr) => lower_match_value_statements(match_expr, ctx, self_type),
        Expr::Return(return_expression) => match return_expression.expr.as_deref() {
            Some(value) => {
                let (mut stmts, value) = lower_expr(value, ctx, self_type)?;
                stmts.push(factorio_ir::statement::Statement::Return(Some(
                    maybe_wrap_return_into(value, ctx),
                )));
                Ok(stmts)
            }
            None => Ok(vec![factorio_ir::statement::Statement::Return(None)]),
        },
        Expr::Continue(_) => Ok(vec![factorio_ir::statement::Statement::Continue]),
        Expr::Break(break_expr) => Ok(vec![lower_break(break_expr)?]),
        _ => {
            let (mut stmts, value) = lower_expr(expression, ctx, self_type)?;
            stmts.push(factorio_ir::statement::Statement::Return(Some(
                maybe_wrap_return_into(value, ctx),
            )));
            Ok(stmts)
        }
    }
}

fn lower_semicolon_statements(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match expression {
        Expr::Return(return_expression) => match return_expression.expr.as_deref() {
            Some(value) => {
                let (mut stmts, value) = lower_expr(value, ctx, self_type)?;
                stmts.push(factorio_ir::statement::Statement::Return(Some(
                    maybe_wrap_return_into(value, ctx),
                )));
                Ok(stmts)
            }
            None => Ok(vec![factorio_ir::statement::Statement::Return(None)]),
        },
        Expr::Assign(assign) => {
            let mark = ctx.try_hoist_mark();
            let stmt = lower_assign_statement(assign, ctx, self_type)?;
            let mut stmts = ctx.take_try_hoists_from(mark);
            stmts.push(stmt);
            Ok(stmts)
        }
        Expr::Binary(binary) if is_compound_assign(&binary.op) => {
            let mark = ctx.try_hoist_mark();
            let stmt = lower_compound_assign_statement(binary, ctx, self_type)?;
            let mut stmts = ctx.take_try_hoists_from(mark);
            stmts.push(stmt);
            Ok(stmts)
        }
        Expr::If(if_expression) => lower_if_expression(if_expression, ctx, self_type),
        Expr::Call(_) | Expr::MethodCall(_) | Expr::Try(_) => {
            let (mut stmts, value) = lower_expr(expression, ctx, self_type)?;
            stmts.push(factorio_ir::statement::Statement::Expr(value));
            Ok(stmts)
        }
        Expr::Macro(mac) => {
            if is_assert_macro(&mac.mac) {
                return lower_assert_macro_statements(mac, ctx, self_type);
            }
            let (mut stmts, value) = lower_expr(expression, ctx, self_type)?;
            stmts.push(factorio_ir::statement::Statement::Expr(value));
            Ok(stmts)
        }
        Expr::Continue(_) => Ok(vec![factorio_ir::statement::Statement::Continue]),
        Expr::Break(break_expr) => Ok(vec![lower_break(break_expr)?]),
        Expr::ForLoop(for_loop) => lower_for_loop(for_loop, ctx, self_type),
        Expr::While(while_expr) => lower_while_loop(while_expr, ctx, self_type),
        Expr::Loop(loop_expr) => lower_infinite_loop(loop_expr, ctx, self_type),
        Expr::Match(match_expr) => lower_match_statements(match_expr, ctx, self_type),
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
    if let Some(range) = for_range_expr(&for_loop.expr) {
        let start = range
            .start
            .as_deref()
            .ok_or_else(|| FrontendError::UnsupportedExpression {
                location: location(range).with_note("range start is required"),
            })?;
        let end = range
            .end
            .as_deref()
            .ok_or_else(|| FrontendError::UnsupportedExpression {
                location: location(range).with_note("range end is required"),
            })?;
        let (mut stmts, start) = lower_expr(start, ctx, self_type)?;
        let (end_hoists, end) = lower_expr(end, ctx, self_type)?;
        stmts.extend(end_hoists);
        let body = lower_block_statements(&for_loop.body.stmts, ctx, self_type)?;
        let limit = match range.limits {
            syn::RangeLimits::Closed(_) => end,
            syn::RangeLimits::HalfOpen(_) => factorio_ir::expression::Expression::BinaryOp {
                lhs: Box::new(end),
                op: factorio_ir::operator::Operator::Sub,
                rhs: Box::new(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Int(1),
                )),
            },
        };
        stmts.push(factorio_ir::statement::Statement::ForNumeric {
            var,
            start,
            limit,
            body,
        });
        return Ok(stmts);
    }

    if let Expr::MethodCall(call) = strip_for_parens(&for_loop.expr)
        && matches!(call.method.to_string().as_str(), "iter" | "into_iter")
        && call.args.is_empty()
    {
        let (mut stmts, iter) = lower_expr(&call.receiver, ctx, self_type)?;
        let body = lower_block_statements(&for_loop.body.stmts, ctx, self_type)?;
        stmts.push(factorio_ir::statement::Statement::ForIn {
            var,
            iter,
            body,
            ipairs: true,
        });
        return Ok(stmts);
    }

    let (mut stmts, iter) = lower_expr(&for_loop.expr, ctx, self_type)?;
    let body = lower_block_statements(&for_loop.body.stmts, ctx, self_type)?;
    stmts.push(factorio_ir::statement::Statement::ForIn {
        ipairs: for_path_is_vec(&for_loop.expr, ctx),
        var,
        iter,
        body,
    });
    Ok(stmts)
}

fn strip_for_parens(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(paren) => strip_for_parens(&paren.expr),
        other => other,
    }
}

fn for_range_expr(expr: &Expr) -> Option<&syn::ExprRange> {
    match strip_for_parens(expr) {
        Expr::Range(range) => Some(range),
        _ => None,
    }
}

fn for_path_is_vec(expr: &Expr, ctx: &LowerContext<'_>) -> bool {
    match strip_for_parens(expr) {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| ctx.binding_type(&segment.ident.to_string()) == Some("Vec")),
        _ => false,
    }
}

fn lower_while_loop(
    while_expr: &syn::ExprWhile,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    lint_option_or_result_condition(while_expr.cond.as_ref(), ctx, "while")?;
    let (mut stmts, condition) = lower_expr(&while_expr.cond, ctx, self_type)?;
    let body = lower_block_statements(&while_expr.body.stmts, ctx, self_type)?;
    stmts.push(factorio_ir::statement::Statement::While { condition, body });
    Ok(stmts)
}

fn lower_infinite_loop(
    loop_expr: &syn::ExprLoop,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let body = lower_block_statements(&loop_expr.body.stmts, ctx, self_type)?;
    Ok(vec![factorio_ir::statement::Statement::While {
        condition: factorio_ir::expression::Expression::Literal(
            factorio_ir::literal::Literal::Bool(true),
        ),
        body,
    }])
}

fn lower_break(break_expr: &syn::ExprBreak) -> FrontendResult<factorio_ir::statement::Statement> {
    if break_expr.expr.is_some() || break_expr.label.is_some() {
        return Err(FrontendError::UnsupportedExpression {
            location: location(break_expr)
                .with_note("only bare `break` is supported (no value or label)"),
        });
    }
    Ok(factorio_ir::statement::Statement::Break)
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
    if matches!(binary.op, BinOp::DivAssign(_))
        && !super::expressions::div_operand_looks_float(&binary.right, ctx)
    {
        ctx.emit_lint(
            factorio_ir::lint::LintId::IntegerDiv,
            "`/=` lowers to Lua `/` (always float); Rust integer `/=` truncates",
            location(binary),
        )?;
    }

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
    let then_block = lower_block_statements(&if_expression.then_branch.stmts, ctx, self_type)?;
    let else_block = match &if_expression.else_branch {
        Some((_, else_branch)) => lower_branch_statements(else_branch, ctx, self_type)?,
        None => Vec::new(),
    };

    let clauses = flatten_and_clauses(if_expression.cond.as_ref());
    if clauses
        .iter()
        .any(|clause| matches!(clause, CondClause::Let { .. }))
    {
        return lower_let_chain_clauses(&clauses, then_block, else_block, ctx, self_type);
    }

    // Plain `if cond` (no `let` bindings in the condition).
    lint_option_or_result_condition(if_expression.cond.as_ref(), ctx, "if")?;
    let (mut stmts, condition) = lower_expr(&if_expression.cond, ctx, self_type)?;
    stmts.push(factorio_ir::statement::Statement::Conditional {
        condition,
        then_block,
        else_block,
    });
    Ok(stmts)
}

fn lint_option_or_result_condition(
    cond: &Expr,
    ctx: &mut LowerContext<'_>,
    keyword: &str,
) -> FrontendResult<()> {
    match cond_surface_binding(cond, ctx) {
        Some("Option") => ctx.emit_lint(
            factorio_ir::lint::LintId::OptionIf,
            format!(
                "`{keyword} option {{ ... }}` uses Lua truthiness (`Some(false)` / `Some(0)` are skipped); use `{keyword} let Some(...)` or `.is_some()`"
            ),
            location(cond),
        ),
        Some("Result") => ctx.emit_lint(
            factorio_ir::lint::LintId::ResultIf,
            format!(
                "`{keyword} result {{ ... }}` is always truthy in Lua (Result is a table); use `{keyword} let Ok(...)` or `.is_ok()`"
            ),
            location(cond),
        ),
        _ => Ok(()),
    }
}

fn cond_surface_binding(cond: &Expr, ctx: &LowerContext<'_>) -> Option<&'static str> {
    match cond {
        Expr::Path(path) if path.path.segments.len() == 1 => ctx
            .binding_surface_type(&path.path.segments[0].ident.to_string())
            .and_then(|key| match key {
                "Option" => Some("Option"),
                "Result" => Some("Result"),
                _ => None,
            }),
        Expr::Paren(paren) => cond_surface_binding(&paren.expr, ctx),
        Expr::Reference(reference) => cond_surface_binding(&reference.expr, ctx),
        _ => None,
    }
}

enum CondClause<'a> {
    /// A normal boolean expression.
    Expr(&'a Expr),
    /// `let Some(name) = expr` / `let Ok(name) = expr` / `let Err(name) = expr` / plain.
    Let {
        kind: LetPatKind,
        binding: String,
        value: &'a Expr,
    },
}

#[derive(Clone, Copy)]
enum LetPatKind {
    /// Option-style: bind value, then `binding ~= nil`.
    OptionSome,
    /// Result Ok: temp Result, `tmp.err == nil`, bind `tmp.ok`.
    ResultOk,
    /// Result Err: temp Result, `tmp.err ~= nil`, bind `tmp.err`.
    ResultErr,
}

fn flatten_and_clauses(expr: &Expr) -> Vec<CondClause<'_>> {
    match expr {
        Expr::Paren(paren) => flatten_and_clauses(&paren.expr),
        Expr::Binary(binary) if matches!(binary.op, BinOp::And(_)) => {
            let mut clauses = flatten_and_clauses(&binary.left);
            clauses.extend(flatten_and_clauses(&binary.right));
            clauses
        }
        Expr::Let(let_expr) => extract_let_pattern(&let_expr.pat).map_or_else(
            || vec![CondClause::Expr(expr)],
            |(kind, binding)| {
                vec![CondClause::Let {
                    kind,
                    binding,
                    value: let_expr.expr.as_ref(),
                }]
            },
        ),
        other => vec![CondClause::Expr(other)],
    }
}

fn lower_let_chain_clauses(
    clauses: &[CondClause<'_>],
    then_block: Vec<factorio_ir::statement::Statement>,
    else_block: Vec<factorio_ir::statement::Statement>,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match clauses {
        [] => Ok(then_block),
        [CondClause::Expr(condition), rest @ ..] => {
            let (mut stmts, condition) = lower_expr(condition, ctx, self_type)?;
            let nested =
                lower_let_chain_clauses(rest, then_block, else_block.clone(), ctx, self_type)?;
            stmts.push(factorio_ir::statement::Statement::Conditional {
                condition,
                then_block: nested,
                else_block,
            });
            Ok(stmts)
        }
        [
            CondClause::Let {
                kind,
                binding,
                value,
            },
            rest @ ..,
        ] => {
            let (mut stmts, rhs) = lower_expr(value, ctx, self_type)?;
            let nested =
                lower_let_chain_clauses(rest, then_block, else_block.clone(), ctx, self_type)?;
            stmts.extend(lower_let_pattern_binding(
                *kind, binding, rhs, nested, else_block, ctx,
            ));
            Ok(stmts)
        }
    }
}

fn lower_let_pattern_binding(
    kind: LetPatKind,
    binding: &str,
    rhs: factorio_ir::expression::Expression,
    then_block: Vec<factorio_ir::statement::Statement>,
    else_block: Vec<factorio_ir::statement::Statement>,
    ctx: &mut LowerContext<'_>,
) -> Vec<factorio_ir::statement::Statement> {
    match kind {
        LetPatKind::OptionSome => lower_let_option_some(binding, rhs, then_block, else_block, ctx),
        LetPatKind::ResultOk => lower_let_result_ok(binding, rhs, then_block, else_block),
        LetPatKind::ResultErr => lower_let_result_err(binding, rhs, then_block, else_block),
    }
}

fn lower_let_option_some(
    binding: &str,
    rhs: factorio_ir::expression::Expression,
    then_block: Vec<factorio_ir::statement::Statement>,
    else_block: Vec<factorio_ir::statement::Statement>,
    ctx: &mut LowerContext<'_>,
) -> Vec<factorio_ir::statement::Statement> {
    if let Some(key) = infer_debug_type_key(&rhs, ctx) {
        ctx.bind_type(binding.to_string(), key);
    }
    vec![
        factorio_ir::statement::Statement::VariableDecl {
            name: binding.to_string(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: rhs,
        },
        factorio_ir::statement::Statement::Conditional {
            condition: factorio_ir::expression::Expression::BinaryOp {
                lhs: Box::new(factorio_ir::expression::Expression::Identifier(
                    binding.to_string(),
                )),
                op: factorio_ir::operator::Operator::Ne,
                rhs: Box::new(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Nil,
                )),
            },
            then_block,
            else_block,
        },
    ]
}

fn lower_let_result_ok(
    binding: &str,
    rhs: factorio_ir::expression::Expression,
    then_block: Vec<factorio_ir::statement::Statement>,
    else_block: Vec<factorio_ir::statement::Statement>,
) -> Vec<factorio_ir::statement::Statement> {
    let tmp = format!("__let_{binding}");
    let mut inner = vec![factorio_ir::statement::Statement::VariableDecl {
        name: binding.to_string(),
        ty: factorio_ir::r#type::Type::Void,
        source_type: None,
        value: factorio_ir::expression::Expression::FieldAccess {
            base: Box::new(factorio_ir::expression::Expression::Identifier(tmp.clone())),
            field: "ok".to_string(),
        },
    }];
    inner.extend(then_block);
    vec![
        factorio_ir::statement::Statement::VariableDecl {
            name: tmp.clone(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: rhs,
        },
        factorio_ir::statement::Statement::Conditional {
            condition: factorio_ir::expression::Expression::BinaryOp {
                lhs: Box::new(factorio_ir::expression::Expression::FieldAccess {
                    base: Box::new(factorio_ir::expression::Expression::Identifier(tmp)),
                    field: "err".to_string(),
                }),
                op: factorio_ir::operator::Operator::Eq,
                rhs: Box::new(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Nil,
                )),
            },
            then_block: inner,
            else_block,
        },
    ]
}

fn lower_let_result_err(
    binding: &str,
    rhs: factorio_ir::expression::Expression,
    then_block: Vec<factorio_ir::statement::Statement>,
    else_block: Vec<factorio_ir::statement::Statement>,
) -> Vec<factorio_ir::statement::Statement> {
    let tmp = format!("__let_{binding}");
    let mut inner = vec![factorio_ir::statement::Statement::VariableDecl {
        name: binding.to_string(),
        ty: factorio_ir::r#type::Type::Void,
        source_type: None,
        value: factorio_ir::expression::Expression::FieldAccess {
            base: Box::new(factorio_ir::expression::Expression::Identifier(tmp.clone())),
            field: "err".to_string(),
        },
    }];
    inner.extend(then_block);
    vec![
        factorio_ir::statement::Statement::VariableDecl {
            name: tmp.clone(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: rhs,
        },
        factorio_ir::statement::Statement::Conditional {
            condition: factorio_ir::expression::Expression::BinaryOp {
                lhs: Box::new(factorio_ir::expression::Expression::FieldAccess {
                    base: Box::new(factorio_ir::expression::Expression::Identifier(tmp)),
                    field: "err".to_string(),
                }),
                op: factorio_ir::operator::Operator::Ne,
                rhs: Box::new(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Nil,
                )),
            },
            then_block: inner,
            else_block,
        },
    ]
}

/// Extract binding kind + name from `Some(x)` / `Ok(x)` / `Err(x)` / plain `x`.
fn extract_let_pattern(pat: &Pat) -> Option<(LetPatKind, String)> {
    match pat {
        Pat::TupleStruct(ts) => {
            let last = ts.path.segments.last()?;
            let kind = match last.ident.to_string().as_str() {
                "Some" => LetPatKind::OptionSome,
                "Ok" => LetPatKind::ResultOk,
                "Err" => LetPatKind::ResultErr,
                _ => return None,
            };
            let inner = ts.elems.first()?;
            Some((kind, extract_plain_binding(inner)?))
        }
        other => Some((LetPatKind::OptionSome, extract_plain_binding(other)?)),
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
        // `else if cond { ... }` is an `Expr::If`, not a block.
        Expr::If(if_expression) => lower_if_expression(if_expression, ctx, self_type),
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

/// Lower `match` in statement position to a temp binding + nested `if`/`else`.
fn lower_match_statements(
    match_expr: &ExprMatch,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let tmp = match_tmp_name(match_expr);
    let (mut stmts, scrutinee) = lower_expr(&match_expr.expr, ctx, self_type)?;
    stmts.push(factorio_ir::statement::Statement::VariableDecl {
        name: tmp.clone(),
        ty: factorio_ir::r#type::Type::Void,
        source_type: None,
        value: scrutinee,
    });
    stmts.extend(fold_match_arms(
        &tmp,
        &match_expr.arms,
        &MatchArmMode::Statement,
        ctx,
        self_type,
    )?);
    Ok(stmts)
}

/// Lower a value-producing `match` in tail/return position (arms `return` directly).
fn lower_match_value_statements(
    match_expr: &ExprMatch,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let tmp = match_tmp_name(match_expr);
    let (mut stmts, scrutinee) = lower_expr(&match_expr.expr, ctx, self_type)?;
    stmts.push(factorio_ir::statement::Statement::VariableDecl {
        name: tmp.clone(),
        ty: factorio_ir::r#type::Type::Void,
        source_type: None,
        value: scrutinee,
    });
    stmts.extend(fold_match_arms(
        &tmp,
        &match_expr.arms,
        &MatchArmMode::Value,
        ctx,
        self_type,
    )?);
    Ok(stmts)
}

/// Lower `match` as a value: hoist statement arms that assign a result temp.
///
/// Any `?` in the scrutinee leaves hoists on `ctx` for the enclosing statement
/// (so `match foo()? { ... }` propagates from the outer function).
pub fn lower_match_expression(
    match_expr: &ExprMatch,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let tmp = match_tmp_name(match_expr);
    let result = ctx.alloc_tmp("match");
    let scrutinee = lower_expression(&match_expr.expr, ctx, self_type)?;
    ctx.try_hoists
        .push(factorio_ir::statement::Statement::VariableDecl {
            name: tmp.clone(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: scrutinee,
        });
    ctx.try_hoists
        .push(factorio_ir::statement::Statement::VariableDecl {
            name: result.clone(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::Nil),
        });
    let arms = fold_match_arms(
        &tmp,
        &match_expr.arms,
        &MatchArmMode::Bind(result.clone()),
        ctx,
        self_type,
    )?;
    ctx.try_hoists.extend(arms);
    Ok(factorio_ir::expression::Expression::Identifier(result))
}

#[derive(Clone)]
enum MatchArmMode {
    Statement,
    Value,
    Bind(String),
}

fn fold_match_arms(
    tmp: &str,
    arms: &[Arm],
    mode: &MatchArmMode,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let scrutinee = factorio_ir::expression::Expression::Identifier(tmp.to_string());
    let mut else_block = Vec::new();
    for arm in arms.iter().rev() {
        let body = lower_match_arm_body(&arm.body, mode, ctx, self_type)?;
        let guard = match &arm.guard {
            Some((_, guard_expr)) => Some(lower_expression(guard_expr, ctx, self_type)?),
            None => None,
        };
        // Top-level `A | B => body` expands to nested arms sharing body/guard so
        // alternatives can bind the same names from different places.
        for alt in flatten_or_patterns(&arm.pat).into_iter().rev() {
            else_block = emit_match_pattern_arm(
                alt,
                &scrutinee,
                guard.clone(),
                body.clone(),
                &else_block,
                ctx,
            )?;
        }
    }

    let tag_tmp = format!("{tmp}_tag");
    if hoist_scrutinee_tag_reads(&mut else_block, &scrutinee, &tag_tmp) {
        let mut out = vec![factorio_ir::statement::Statement::VariableDecl {
            name: tag_tmp,
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: factorio_ir::expression::Expression::FieldAccess {
                base: Box::new(scrutinee),
                field: "tag".to_string(),
            },
        }];
        out.extend(else_block);
        Ok(out)
    } else {
        Ok(else_block)
    }
}

fn emit_match_pattern_arm(
    pat: &Pat,
    scrutinee: &factorio_ir::expression::Expression,
    guard: Option<factorio_ir::expression::Expression>,
    body: Vec<factorio_ir::statement::Statement>,
    else_block: &[factorio_ir::statement::Statement],
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let (condition, bindings) = lower_match_pattern(pat, scrutinee, ctx)?;
    let mut then_block = Vec::new();
    let mut bound_fields = Vec::new();
    for (name, value) in bindings {
        if let Some(key) = infer_debug_type_key(&value, ctx) {
            ctx.bind_type(name.clone(), key);
        }
        bound_fields.push((name.clone(), value.clone()));
        then_block.push(factorio_ir::statement::Statement::VariableDecl {
            name,
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value,
        });
    }

    if let Some(guard) = guard {
        let guard_fail_else =
            fallthrough_after_guard_fail(else_block, condition.as_ref(), &bound_fields);
        then_block.push(factorio_ir::statement::Statement::Conditional {
            condition: guard,
            then_block: body,
            else_block: guard_fail_else,
        });
    } else {
        then_block.extend(body);
    }

    Ok(match condition {
        None => then_block,
        Some(condition) => {
            let pattern_miss_else = fallthrough_after_pattern_miss(else_block, &condition);
            vec![factorio_ir::statement::Statement::Conditional {
                condition,
                then_block,
                else_block: pattern_miss_else,
            }]
        }
    })
}

fn fallthrough_after_guard_fail(
    else_block: &[factorio_ir::statement::Statement],
    matched_condition: Option<&factorio_ir::expression::Expression>,
    already_bound: &[(String, factorio_ir::expression::Expression)],
) -> Vec<factorio_ir::statement::Statement> {
    let Some(matched) = matched_condition else {
        return else_block.to_vec();
    };
    match else_block {
        [
            factorio_ir::statement::Statement::Conditional {
                condition: next_cond,
                then_block,
                else_block: nested,
            },
        ] if match_conditions_equiv(matched, next_cond) => {
            let _ = nested;
            strip_redundant_pattern_bindings(then_block.clone(), already_bound)
        }
        _ => else_block.to_vec(),
    }
}

fn strip_redundant_pattern_bindings(
    stmts: Vec<factorio_ir::statement::Statement>,
    already_bound: &[(String, factorio_ir::expression::Expression)],
) -> Vec<factorio_ir::statement::Statement> {
    stmts
        .into_iter()
        .filter(|statement| match statement {
            factorio_ir::statement::Statement::VariableDecl { name, value, .. } => !already_bound
                .iter()
                .any(|(bound_name, bound_value)| bound_name == name && bound_value == value),
            _ => true,
        })
        .collect()
}

fn hoist_scrutinee_tag_reads(
    stmts: &mut [factorio_ir::statement::Statement],
    scrutinee: &factorio_ir::expression::Expression,
    tag_tmp: &str,
) -> bool {
    let mut changed = false;
    for statement in stmts.iter_mut() {
        changed |= hoist_scrutinee_tag_in_statement(statement, scrutinee, tag_tmp);
    }
    changed
}

fn hoist_scrutinee_tag_in_statement(
    statement: &mut factorio_ir::statement::Statement,
    scrutinee: &factorio_ir::expression::Expression,
    tag_tmp: &str,
) -> bool {
    match statement {
        factorio_ir::statement::Statement::Conditional {
            condition,
            then_block,
            else_block,
        } => {
            let mut changed = replace_scrutinee_tag_expr(condition, scrutinee, tag_tmp);
            for statement in then_block.iter_mut().chain(else_block.iter_mut()) {
                changed |= hoist_scrutinee_tag_in_statement(statement, scrutinee, tag_tmp);
            }
            changed
        }
        factorio_ir::statement::Statement::VariableDecl { value, .. }
        | factorio_ir::statement::Statement::Return(Some(value))
        | factorio_ir::statement::Statement::Expr(value) => {
            replace_scrutinee_tag_expr(value, scrutinee, tag_tmp)
        }
        factorio_ir::statement::Statement::Assignment { target, value } => {
            replace_scrutinee_tag_expr(target, scrutinee, tag_tmp)
                | replace_scrutinee_tag_expr(value, scrutinee, tag_tmp)
        }
        factorio_ir::statement::Statement::ForIn { iter, body, .. } => {
            let mut changed = replace_scrutinee_tag_expr(iter, scrutinee, tag_tmp);
            for statement in body {
                changed |= hoist_scrutinee_tag_in_statement(statement, scrutinee, tag_tmp);
            }
            changed
        }
        factorio_ir::statement::Statement::ForNumeric {
            start, limit, body, ..
        } => {
            let mut changed = replace_scrutinee_tag_expr(start, scrutinee, tag_tmp);
            changed |= replace_scrutinee_tag_expr(limit, scrutinee, tag_tmp);
            for statement in body {
                changed |= hoist_scrutinee_tag_in_statement(statement, scrutinee, tag_tmp);
            }
            changed
        }
        factorio_ir::statement::Statement::While { condition, body } => {
            let mut changed = replace_scrutinee_tag_expr(condition, scrutinee, tag_tmp);
            for statement in body {
                changed |= hoist_scrutinee_tag_in_statement(statement, scrutinee, tag_tmp);
            }
            changed
        }
        factorio_ir::statement::Statement::FunctionDecl(_)
        | factorio_ir::statement::Statement::StructDecl(_)
        | factorio_ir::statement::Statement::EnumDecl(_)
        | factorio_ir::statement::Statement::Return(None)
        | factorio_ir::statement::Statement::Continue
        | factorio_ir::statement::Statement::Break => false,
    }
}

fn replace_scrutinee_tag_expr(
    expr: &mut factorio_ir::expression::Expression,
    scrutinee: &factorio_ir::expression::Expression,
    tag_tmp: &str,
) -> bool {
    if is_scrutinee_tag(expr, scrutinee) {
        *expr = factorio_ir::expression::Expression::Identifier(tag_tmp.to_string());
        return true;
    }
    match expr {
        factorio_ir::expression::Expression::FieldAccess { base, .. }
        | factorio_ir::expression::Expression::Not(base)
        | factorio_ir::expression::Expression::Len(base)
        | factorio_ir::expression::Expression::FatPointer { data: base, .. } => {
            replace_scrutinee_tag_expr(base, scrutinee, tag_tmp)
        }
        factorio_ir::expression::Expression::BinaryOp { lhs, rhs, .. }
        | factorio_ir::expression::Expression::Index {
            base: lhs,
            key: rhs,
        } => {
            replace_scrutinee_tag_expr(lhs, scrutinee, tag_tmp)
                | replace_scrutinee_tag_expr(rhs, scrutinee, tag_tmp)
        }
        factorio_ir::expression::Expression::Call { func, args } => {
            let mut changed = replace_scrutinee_tag_expr(func, scrutinee, tag_tmp);
            for arg in args {
                changed |= replace_scrutinee_tag_expr(arg, scrutinee, tag_tmp);
            }
            changed
        }
        factorio_ir::expression::Expression::MethodCall { receiver, args, .. }
        | factorio_ir::expression::Expression::DynMethodCall { receiver, args, .. } => {
            let mut changed = replace_scrutinee_tag_expr(receiver, scrutinee, tag_tmp);
            for arg in args {
                changed |= replace_scrutinee_tag_expr(arg, scrutinee, tag_tmp);
            }
            changed
        }
        factorio_ir::expression::Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            replace_scrutinee_tag_expr(condition, scrutinee, tag_tmp)
                | replace_scrutinee_tag_expr(then_expr, scrutinee, tag_tmp)
                | replace_scrutinee_tag_expr(else_expr, scrutinee, tag_tmp)
        }
        factorio_ir::expression::Expression::FormatConcat { parts }
        | factorio_ir::expression::Expression::Array { elements: parts } => {
            let mut changed = false;
            for part in parts {
                changed |= replace_scrutinee_tag_expr(part, scrutinee, tag_tmp);
            }
            changed
        }
        factorio_ir::expression::Expression::StructLiteral { fields, .. }
        | factorio_ir::expression::Expression::EnumLiteral { fields, .. } => {
            let mut changed = false;
            for (_, value) in fields {
                changed |= replace_scrutinee_tag_expr(value, scrutinee, tag_tmp);
            }
            changed
        }
        factorio_ir::expression::Expression::Closure { body, .. } => {
            hoist_scrutinee_tag_reads(&mut body.statements, scrutinee, tag_tmp)
        }
        factorio_ir::expression::Expression::Literal(_)
        | factorio_ir::expression::Expression::Identifier(_)
        | factorio_ir::expression::Expression::QualifiedPath { .. } => false,
    }
}

fn is_scrutinee_tag(
    expr: &factorio_ir::expression::Expression,
    scrutinee: &factorio_ir::expression::Expression,
) -> bool {
    matches!(
        expr,
        factorio_ir::expression::Expression::FieldAccess { base, field }
            if field == "tag" && base.as_ref() == scrutinee
    )
}

fn fallthrough_after_pattern_miss(
    else_block: &[factorio_ir::statement::Statement],
    failed_condition: &factorio_ir::expression::Expression,
) -> Vec<factorio_ir::statement::Statement> {
    let mut block = else_block.to_vec();
    while let [
        factorio_ir::statement::Statement::Conditional {
            condition: next_cond,
            else_block: nested,
            ..
        },
    ] = block.as_slice()
    {
        if match_conditions_equiv(failed_condition, next_cond) {
            block = nested.clone();
        } else {
            break;
        }
    }
    block
}

fn match_conditions_equiv(
    a: &factorio_ir::expression::Expression,
    b: &factorio_ir::expression::Expression,
) -> bool {
    a == b
}

fn lower_match_arm_body(
    body: &Expr,
    mode: &MatchArmMode,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match mode {
        MatchArmMode::Statement => match body {
            Expr::Block(block) => lower_block_statements(&block.block.stmts, ctx, self_type),
            other => lower_semicolon_statements(other, ctx, self_type),
        },
        MatchArmMode::Value => match body {
            Expr::Block(block) => Ok(lower_block(&block.block, ctx, self_type)?.statements),
            other => {
                let (mut stmts, value) = lower_expr(other, ctx, self_type)?;
                stmts.push(factorio_ir::statement::Statement::Return(Some(
                    maybe_wrap_return_into(value, ctx),
                )));
                Ok(stmts)
            }
        },
        MatchArmMode::Bind(name) => lower_match_arm_bind_body(body, name, ctx, self_type),
    }
}

fn lower_match_arm_bind_body(
    body: &Expr,
    name: &str,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match body {
        Expr::Block(block) => {
            let stmts = &block.block.stmts;
            if stmts.is_empty() {
                return Ok(vec![factorio_ir::statement::Statement::Assignment {
                    target: factorio_ir::expression::Expression::Identifier(name.to_string()),
                    value: factorio_ir::expression::Expression::Literal(
                        factorio_ir::literal::Literal::Nil,
                    ),
                }]);
            }
            let mut out = Vec::new();
            let last = stmts.len() - 1;
            for (index, statement) in stmts.iter().enumerate() {
                if index != last {
                    out.extend(lower_statement(statement, false, ctx, self_type)?);
                    continue;
                }
                match statement {
                    Stmt::Expr(Expr::Return(_), _) => {
                        out.extend(lower_statement(statement, false, ctx, self_type)?);
                    }
                    Stmt::Expr(expression, None) => {
                        let (mut hoists, value) = lower_expr(expression, ctx, self_type)?;
                        out.append(&mut hoists);
                        out.push(factorio_ir::statement::Statement::Assignment {
                            target: factorio_ir::expression::Expression::Identifier(
                                name.to_string(),
                            ),
                            value,
                        });
                    }
                    other => {
                        out.extend(lower_statement(other, false, ctx, self_type)?);
                        out.push(factorio_ir::statement::Statement::Assignment {
                            target: factorio_ir::expression::Expression::Identifier(
                                name.to_string(),
                            ),
                            value: factorio_ir::expression::Expression::Literal(
                                factorio_ir::literal::Literal::Nil,
                            ),
                        });
                    }
                }
            }
            Ok(out)
        }
        other => {
            let (mut stmts, value) = lower_expr(other, ctx, self_type)?;
            stmts.push(factorio_ir::statement::Statement::Assignment {
                target: factorio_ir::expression::Expression::Identifier(name.to_string()),
                value,
            });
            Ok(stmts)
        }
    }
}

fn flatten_or_patterns(pat: &Pat) -> Vec<&Pat> {
    match pat {
        Pat::Or(or_pat) => or_pat.cases.iter().flat_map(flatten_or_patterns).collect(),
        Pat::Paren(paren) => flatten_or_patterns(&paren.pat),
        other => vec![other],
    }
}

type MatchPatternParts = (
    Option<factorio_ir::expression::Expression>,
    Vec<(String, factorio_ir::expression::Expression)>,
);

/// Returns `(condition, bindings)`. `None` condition is irrefutable (`_` / plain ident).
fn lower_match_pattern(
    pat: &Pat,
    scrutinee: &factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> FrontendResult<MatchPatternParts> {
    match pat {
        Pat::Wild(_) => Ok((None, vec![])),
        Pat::Ident(ident) if ident.ident == "None" => Ok((Some(eq_nil(scrutinee.clone())), vec![])),
        Pat::Ident(ident) => Ok((
            None,
            vec![(ident.ident.to_string(), scrutinee.clone())],
        )),
        Pat::Lit(lit) => Ok((
            Some(eq_expr(scrutinee.clone(), literal_from_pat_lit(lit, pat)?)),
            vec![],
        )),
        Pat::Path(path) if is_none_path(&path.path) => {
            Ok((Some(eq_nil(scrutinee.clone())), vec![]))
        }
        Pat::Path(path) => lower_enum_unit_pattern(path, scrutinee, ctx),
        Pat::TupleStruct(ts) if enum_pattern_variant(&ts.path, ctx).is_some() => {
            lower_enum_tuple_pattern(ts, scrutinee, ctx)
        }
        Pat::TupleStruct(ts) if is_some_path(&ts.path) => {
            let inner = ts.elems.first().ok_or_else(|| FrontendError::UnsupportedExpression {
                location: location(pat).with_note("Some(...) pattern requires one binding"),
            })?;
            let (inner_cond, inner_binds) = lower_match_pattern(inner, scrutinee, ctx)?;
            Ok((
                Some(and_conditions(ne_nil(scrutinee.clone()), inner_cond)),
                inner_binds,
            ))
        }
        Pat::TupleStruct(ts) if is_ok_path(&ts.path) => {
            let inner = ts.elems.first().ok_or_else(|| FrontendError::UnsupportedExpression {
                location: location(pat).with_note("Ok(...) pattern requires one binding"),
            })?;
            let ok_field = factorio_ir::expression::Expression::FieldAccess {
                base: Box::new(scrutinee.clone()),
                field: "ok".to_string(),
            };
            let (inner_cond, inner_binds) = lower_match_pattern(inner, &ok_field, ctx)?;
            let is_ok = eq_nil(factorio_ir::expression::Expression::FieldAccess {
                base: Box::new(scrutinee.clone()),
                field: "err".to_string(),
            });
            Ok((Some(and_conditions(is_ok, inner_cond)), inner_binds))
        }
        Pat::TupleStruct(ts) if is_err_path(&ts.path) => {
            let inner = ts.elems.first().ok_or_else(|| FrontendError::UnsupportedExpression {
                location: location(pat).with_note("Err(...) pattern requires one binding"),
            })?;
            let err_field = factorio_ir::expression::Expression::FieldAccess {
                base: Box::new(scrutinee.clone()),
                field: "err".to_string(),
            };
            let (inner_cond, inner_binds) = lower_match_pattern(inner, &err_field, ctx)?;
            let is_err = ne_nil(err_field);
            Ok((Some(and_conditions(is_err, inner_cond)), inner_binds))
        }
        Pat::Struct(struct_pat) if enum_pattern_variant(&struct_pat.path, ctx).is_some() => {
            lower_enum_struct_pattern(struct_pat, scrutinee, ctx)
        }
        // Cross-module `Enum::Variant { .. }` when the enum was not defined in this
        // file: still emit a tag check. Falling through to a bare struct pattern
        // with only `..` would match unconditionally (always `true`).
        Pat::Struct(struct_pat) if is_enum_variant_path(&struct_pat.path) => {
            lower_cross_module_enum_struct_pattern(struct_pat, scrutinee, ctx)
        }
        Pat::Struct(struct_pat) => lower_struct_pattern(struct_pat, scrutinee, ctx),
        Pat::Or(or_pat) => lower_nested_or_pattern(or_pat, scrutinee, ctx),
        Pat::Type(pat_type) => lower_match_pattern(&pat_type.pat, scrutinee, ctx),
        Pat::Paren(paren) => lower_match_pattern(&paren.pat, scrutinee, ctx),
        Pat::Reference(reference) => lower_match_pattern(&reference.pat, scrutinee, ctx),
        _ => Err(FrontendError::UnsupportedExpression {
            location: location(pat).with_note(
                "match supports `_`, literals, `None`, `Some(...)`, struct and enum patterns, or-patterns, and plain bindings",
            ),
        }),
    }
}

fn lower_struct_pattern(
    struct_pat: &syn::PatStruct,
    scrutinee: &factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> FrontendResult<MatchPatternParts> {
    let mut condition = None;
    let mut bindings = Vec::new();
    for field in &struct_pat.fields {
        let field_name = match &field.member {
            syn::Member::Named(ident) => ident.to_string(),
            syn::Member::Unnamed(index) => {
                return Err(FrontendError::UnsupportedExpression {
                    location: location(index)
                        .with_note("tuple struct field patterns are not supported"),
                });
            }
        };
        let field_scrutinee = factorio_ir::expression::Expression::FieldAccess {
            base: Box::new(scrutinee.clone()),
            field: field_name,
        };
        let (field_cond, field_binds) = lower_match_pattern(&field.pat, &field_scrutinee, ctx)?;
        condition = match (condition, field_cond) {
            (None, c) => c,
            (Some(left), Some(right)) => Some(and_expr(left, right)),
            (Some(left), None) => Some(left),
        };
        bindings.extend(field_binds);
    }
    // `..` / rest is ignored - Lua tables have no exhaustiveness check.
    Ok((condition, bindings))
}

fn enum_pattern_variant(
    path: &syn::Path,
    ctx: &LowerContext<'_>,
) -> Option<(String, String, super::context::EnumVariantFields)> {
    let variant = path.segments.last()?.ident.to_string();
    let enum_name = path.segments.iter().nth_back(1)?.ident.to_string();
    if enum_name == "Self" {
        return ctx.enums.iter().find_map(|(name, variants)| {
            variants
                .iter()
                .find(|info| info.name == variant)
                .map(|info| (name.clone(), variant.clone(), info.fields))
        });
    }
    ctx.enum_variant(&enum_name, &variant)
        .map(|fields| (enum_name, variant, fields))
}

fn enum_tag_condition(
    scrutinee: &factorio_ir::expression::Expression,
    variant: String,
) -> factorio_ir::expression::Expression {
    eq_expr(
        factorio_ir::expression::Expression::FieldAccess {
            base: Box::new(scrutinee.clone()),
            field: "tag".to_string(),
        },
        factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::String(
            variant,
        )),
    )
}

fn lower_enum_unit_pattern(
    path: &syn::PatPath,
    scrutinee: &factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> FrontendResult<MatchPatternParts> {
    let Some((_, variant, fields)) = enum_pattern_variant(&path.path, ctx) else {
        return Err(FrontendError::UnsupportedExpression {
            location: location(path),
        });
    };
    if fields != super::context::EnumVariantFields::Unit {
        return Err(FrontendError::UnsupportedExpression {
            location: location(path).with_note("enum variant payload must be matched"),
        });
    }
    Ok((Some(enum_tag_condition(scrutinee, variant)), vec![]))
}

fn lower_enum_tuple_pattern(
    pattern: &syn::PatTupleStruct,
    scrutinee: &factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> FrontendResult<MatchPatternParts> {
    let Some((_, variant, fields)) = enum_pattern_variant(&pattern.path, ctx) else {
        return Err(FrontendError::UnsupportedExpression {
            location: location(pattern),
        });
    };
    let super::context::EnumVariantFields::Tuple(count) = fields else {
        return Err(FrontendError::UnsupportedExpression {
            location: location(pattern).with_note("enum variant is not a tuple variant"),
        });
    };
    if pattern.elems.len() != count {
        return Err(FrontendError::UnsupportedExpression {
            location: location(pattern)
                .with_note(format!("enum tuple variant expects {count} fields")),
        });
    }
    let mut condition = Some(enum_tag_condition(scrutinee, variant));
    let mut bindings = Vec::new();
    for (index, pat) in pattern.elems.iter().enumerate() {
        let field = factorio_ir::expression::Expression::FieldAccess {
            base: Box::new(scrutinee.clone()),
            field: format!("_{}", index + 1),
        };
        let (field_cond, field_binds) = lower_match_pattern(pat, &field, ctx)?;
        condition = match (condition, field_cond) {
            (Some(left), Some(right)) => Some(and_expr(left, right)),
            (left, None) => left,
            (None, right) => right,
        };
        bindings.extend(field_binds);
    }
    Ok((condition, bindings))
}

fn lower_enum_struct_pattern(
    pattern: &syn::PatStruct,
    scrutinee: &factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> FrontendResult<MatchPatternParts> {
    let Some((_, variant, fields)) = enum_pattern_variant(&pattern.path, ctx) else {
        return Err(FrontendError::UnsupportedExpression {
            location: location(pattern),
        });
    };
    if fields != super::context::EnumVariantFields::Named {
        return Err(FrontendError::UnsupportedExpression {
            location: location(pattern).with_note("enum variant is not a struct variant"),
        });
    }
    let (field_condition, bindings) = lower_struct_pattern(pattern, scrutinee, ctx)?;
    Ok((
        Some(and_conditions(
            enum_tag_condition(scrutinee, variant),
            field_condition,
        )),
        bindings,
    ))
}

/// `Enum::Variant { .. }` / `{ field }` when the enum lives in another module.
fn lower_cross_module_enum_struct_pattern(
    pattern: &syn::PatStruct,
    scrutinee: &factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> FrontendResult<MatchPatternParts> {
    let variant = pattern
        .path
        .segments
        .last()
        .ok_or_else(|| FrontendError::UnsupportedExpression {
            location: location(pattern),
        })?
        .ident
        .to_string();
    let (field_condition, bindings) = lower_struct_pattern(pattern, scrutinee, ctx)?;
    Ok((
        Some(and_conditions(
            enum_tag_condition(scrutinee, variant),
            field_condition,
        )),
        bindings,
    ))
}

/// `Type::Variant` paths used as enum patterns (including cross-module imports).
fn is_enum_variant_path(path: &syn::Path) -> bool {
    if path.segments.len() < 2 {
        return false;
    }
    let enum_name = path
        .segments
        .iter()
        .nth_back(1)
        .map(|s| s.ident.to_string());
    !matches!(enum_name.as_deref(), Some("Option" | "Result"))
}

fn lower_nested_or_pattern(
    or_pat: &syn::PatOr,
    scrutinee: &factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> FrontendResult<MatchPatternParts> {
    let mut condition = None;
    let mut any_irrefutable = false;
    let mut bindings: Option<Vec<(String, factorio_ir::expression::Expression)>> = None;
    for case in &or_pat.cases {
        let (case_cond, case_binds) = lower_match_pattern(case, scrutinee, ctx)?;
        match &mut bindings {
            None => bindings = Some(case_binds),
            Some(existing) => {
                if existing != &case_binds {
                    return Err(FrontendError::UnsupportedExpression {
                        location: location(or_pat).with_note(
                            "nested or-pattern alternatives must bind the same names the same way; use separate match arms",
                        ),
                    });
                }
            }
        }
        match case_cond {
            None => any_irrefutable = true,
            Some(case_cond) => {
                condition = Some(match condition {
                    Some(left) => or_expr(left, case_cond),
                    None => case_cond,
                });
            }
        }
    }
    let condition = if any_irrefutable { None } else { condition };
    Ok((condition, bindings.unwrap_or_default()))
}

fn literal_from_pat_lit(
    lit: &syn::PatLit,
    pat: &Pat,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let literal = match &lit.lit {
        Lit::Int(value) => {
            let parsed = value.base10_parse::<i64>().map_err(FrontendError::from)?;
            factorio_ir::literal::Literal::Int(parsed)
        }
        Lit::Float(value) => {
            let parsed = value.base10_parse::<f64>().map_err(FrontendError::from)?;
            factorio_ir::literal::Literal::Float(parsed)
        }
        Lit::Str(value) => factorio_ir::literal::Literal::String(value.value()),
        Lit::Bool(value) => factorio_ir::literal::Literal::Bool(value.value),
        _ => {
            return Err(FrontendError::UnsupportedExpression {
                location: location(pat).with_note("unsupported match literal"),
            });
        }
    };
    Ok(factorio_ir::expression::Expression::Literal(literal))
}

fn eq_expr(
    lhs: factorio_ir::expression::Expression,
    rhs: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(lhs),
        op: factorio_ir::operator::Operator::Eq,
        rhs: Box::new(rhs),
    }
}

fn eq_nil(expr: factorio_ir::expression::Expression) -> factorio_ir::expression::Expression {
    eq_expr(
        expr,
        factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::Nil),
    )
}

fn ne_nil(expr: factorio_ir::expression::Expression) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(expr),
        op: factorio_ir::operator::Operator::Ne,
        rhs: Box::new(factorio_ir::expression::Expression::Literal(
            factorio_ir::literal::Literal::Nil,
        )),
    }
}

fn and_expr(
    lhs: factorio_ir::expression::Expression,
    rhs: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(lhs),
        op: factorio_ir::operator::Operator::And,
        rhs: Box::new(rhs),
    }
}

fn and_conditions(
    left: factorio_ir::expression::Expression,
    right: Option<factorio_ir::expression::Expression>,
) -> factorio_ir::expression::Expression {
    match right {
        Some(right) => and_expr(left, right),
        None => left,
    }
}

fn or_expr(
    lhs: factorio_ir::expression::Expression,
    rhs: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(lhs),
        op: factorio_ir::operator::Operator::Or,
        rhs: Box::new(rhs),
    }
}

fn is_none_path(path: &syn::Path) -> bool {
    path.segments.last().is_some_and(|seg| seg.ident == "None")
}

fn is_some_path(path: &syn::Path) -> bool {
    path.segments.last().is_some_and(|seg| seg.ident == "Some")
}

fn is_ok_path(path: &syn::Path) -> bool {
    path.segments.last().is_some_and(|seg| seg.ident == "Ok")
}

fn is_err_path(path: &syn::Path) -> bool {
    path.segments.last().is_some_and(|seg| seg.ident == "Err")
}

fn match_tmp_name(match_expr: &ExprMatch) -> String {
    use syn::spanned::Spanned;
    format!("__match_{}", match_expr.span().byte_range().start)
}

/// Lower `matches!(expr, pat)` / `matches!(expr, pat if guard)` to a value `match`.
///
/// Desugars to `match expr { pat if guard => true, _ => false }`, then uses the
/// same arm folding as ordinary value-position `match`.
pub fn lower_matches_macro(
    mac: &syn::ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let input: MatchesMacroInput = mac.mac.parse_body()?;
    let expr = &input.expr;
    let pat = &input.pat;
    let match_expr: ExprMatch = input.guard.as_ref().map_or_else(
        || {
            syn::parse_quote! {
                match #expr {
                    #pat => true,
                    _ => false,
                }
            }
        },
        |guard| {
            syn::parse_quote! {
                match #expr {
                    #pat if #guard => true,
                    _ => false,
                }
            }
        },
    );
    lower_match_expression(&match_expr, ctx, self_type)
}

struct MatchesMacroInput {
    expr: Expr,
    pat: Pat,
    guard: Option<Expr>,
}

impl syn::parse::Parse for MatchesMacroInput {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        // Avoid eager braces
        let expr = Expr::parse_without_eager_brace(input)?;
        input.parse::<syn::Token![,]>()?;
        let pat = Pat::parse_multi(input)?;
        let guard = if input.peek(syn::Token![if]) {
            input.parse::<syn::Token![if]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        if input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
        }
        Ok(Self { expr, pat, guard })
    }
}

/// Split `__vt_Trait_Concrete` into `(Trait, Concrete)`.
fn parse_vtable_parts(vtable: &str) -> Option<(String, String)> {
    let rest = vtable.strip_prefix("__vt_")?;
    let (trait_name, concrete) = rest.split_once('_')?;
    if trait_name.is_empty() || concrete.is_empty() {
        return None;
    }
    Some((trait_name.to_string(), concrete.to_string()))
}

fn bind_let_local_type(
    name: &str,
    pat: &Pat,
    value: &factorio_ir::expression::Expression,
    init_expr: &Expr,
    ctx: &mut LowerContext<'_>,
) {
    if let syn::Pat::Type(pat_type) = pat {
        bind_typed_local(name, &pat_type.ty, value, init_expr, ctx);
        return;
    }
    if let factorio_ir::expression::Expression::FatPointer { vtable, .. } = value {
        if let Some((trait_name, concrete)) = parse_vtable_parts(vtable) {
            ctx.bind_dyn(
                name.to_string(),
                super::traits::dyn_local(trait_name, concrete),
            );
        }
        return;
    }
    if let factorio_ir::expression::Expression::EnumLiteral { enum_name, .. } = value {
        ctx.user_structs.insert(enum_name.clone());
        ctx.bind_type(name.to_string(), enum_name.clone());
        return;
    }
    if let factorio_ir::expression::Expression::QualifiedPath { segments } = value
        && segments.len() >= 2
        && let Some(enum_name) = segments.iter().nth_back(1)
        && ctx.is_user_struct(enum_name)
    {
        ctx.bind_type(name.to_string(), enum_name.clone());
        return;
    }
    // `let x = opt.unwrap_or(Phase::Idle)` / `storage.get::<Phase>(...)` — keep the
    // concrete type so later `x.tick()` lowers as `Phase.tick(x)`, not a property read.
    if let Some(key) = type_key_from_rust_expr(init_expr, ctx) {
        ctx.bind_type(name.to_string(), key);
        return;
    }
    if let Some(key) = infer_debug_type_key(value, ctx) {
        ctx.bind_type(name.to_string(), key);
    }
}

fn type_key_from_rust_expr(expr: &Expr, ctx: &LowerContext<'_>) -> Option<String> {
    match strip_expr_parens(expr) {
        Expr::MethodCall(call) => {
            let method = call.method.to_string();
            if matches!(method.as_str(), "unwrap_or" | "or") && call.args.len() == 1 {
                return type_key_from_rust_expr(&call.args[0], ctx)
                    .or_else(|| type_key_from_method_turbofish(call, ctx))
                    .or_else(|| type_key_from_rust_expr(&call.receiver, ctx));
            }
            if method == "get" {
                return type_key_from_method_turbofish(call, ctx);
            }
            None
        }
        Expr::Path(path) => user_type_owner_from_path(&path.path, ctx),
        Expr::Call(call) => match call.func.as_ref() {
            Expr::Path(path) => user_type_owner_from_path(&path.path, ctx),
            _ => None,
        },
        Expr::Struct(item) => {
            let name = item.path.segments.last()?.ident.to_string();
            ctx.is_user_struct(&name).then_some(name)
        }
        _ => None,
    }
}

fn type_key_from_method_turbofish(
    call: &syn::ExprMethodCall,
    ctx: &LowerContext<'_>,
) -> Option<String> {
    let turbofish = call.turbofish.as_ref()?;
    for arg in &turbofish.args {
        if let syn::GenericArgument::Type(ty) = arg
            && let Some(key) = rust_type_key(ty, &ctx.type_aliases, &ctx.assoc_bindings)
            && ctx.is_user_struct(&key)
        {
            return Some(key);
        }
    }
    None
}

fn user_type_owner_from_path(path: &syn::Path, ctx: &LowerContext<'_>) -> Option<String> {
    if path.segments.len() >= 2 {
        let owner = path.segments[path.segments.len() - 2].ident.to_string();
        if ctx.is_user_struct(&owner) {
            return Some(owner);
        }
    }
    let name = path.segments.last()?.ident.to_string();
    ctx.is_user_struct(&name).then_some(name)
}

fn strip_expr_parens(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(paren) => strip_expr_parens(&paren.expr),
        Expr::Group(group) => strip_expr_parens(&group.expr),
        other => other,
    }
}

fn bind_typed_local(
    name: &str,
    ty: &syn::Type,
    value: &factorio_ir::expression::Expression,
    init_expr: &Expr,
    ctx: &mut LowerContext<'_>,
) {
    if let Some(key) = rust_type_key(ty, &ctx.type_aliases, &ctx.assoc_bindings) {
        ctx.bind_type(name.to_string(), key);
    }
    if let factorio_ir::expression::Expression::EnumLiteral { enum_name, .. } = value {
        ctx.user_structs.insert(enum_name.clone());
        if ctx.binding_type(name).is_none() {
            ctx.bind_type(name.to_string(), enum_name.clone());
        }
    }
    if is_option_type(ty, &ctx.type_aliases, &ctx.assoc_bindings) {
        ctx.bind_option(name.to_string());
    }
    if let Some(trait_name) = super::traits::dyn_trait_name(ty) {
        let concrete = match value {
            factorio_ir::expression::Expression::FatPointer { vtable, .. } => {
                parse_vtable_parts(vtable)
                    .map_or_else(|| "Unknown".to_string(), |(_, concrete)| concrete)
            }
            _ => super::traits::resolve_concrete_type(init_expr, ctx)
                .unwrap_or_else(|| "Unknown".to_string()),
        };
        ctx.bind_dyn(
            name.to_string(),
            super::traits::dyn_local(trait_name, concrete),
        );
    }
}
