use syn::{BinOp, Expr, ExprBinary, ExprLit, ExprPath, Lit, Member, UnOp};

use crate::error::{FrontendError, FrontendResult};

use super::{context::LowerContext, print::lower_macro_expression, util::location};

pub fn lower_expression(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    match expression {
        Expr::Binary(binary) => lower_binary_expression(binary, ctx, self_type),
        Expr::Lit(literal) => lower_literal_expression(literal),
        Expr::Path(path) => lower_path_expression(path, ctx, self_type),
        Expr::Field(field) => lower_field_expression(field, ctx, self_type),
        Expr::Call(call) => {
            let func = lower_expression(&call.func, ctx, self_type)?;
            let args = call
                .args
                .iter()
                .map(|arg| lower_expression(arg, ctx, self_type))
                .collect::<FrontendResult<Vec<_>>>()?;
            Ok(factorio_ir::expression::Expression::Call {
                func: Box::new(func),
                args,
            })
        }
        Expr::MethodCall(call) => lower_method_call(call, ctx, self_type),
        Expr::Struct(item) => lower_struct_expression(item, ctx, self_type),
        Expr::Macro(mac) => lower_macro_expression(mac, ctx, self_type),
        Expr::Array(array) => {
            let elements = array
                .elems
                .iter()
                .map(|elem| lower_expression(elem, ctx, self_type))
                .collect::<FrontendResult<Vec<_>>>()?;
            Ok(factorio_ir::expression::Expression::Array { elements })
        }
        Expr::Index(index) => {
            let base = lower_expression(&index.expr, ctx, self_type)?;
            let key = lower_expression(&index.index, ctx, self_type)?;
            Ok(factorio_ir::expression::Expression::Index {
                base: Box::new(base),
                key: Box::new(key),
            })
        }
        Expr::Reference(reference) => lower_expression(&reference.expr, ctx, self_type),
        // `x as T` - Lua has no casts; lower the inner value.
        Expr::Cast(cast) => lower_expression(&cast.expr, ctx, self_type),
        // `(expr)` - transparent grouping.
        Expr::Paren(paren) => lower_expression(&paren.expr, ctx, self_type),
        // `if cond { a } else { b }` as an expression -> Lua `cond and a or b` ternary idiom.
        Expr::If(if_expr) => lower_if_expr(if_expr, ctx, self_type),
        Expr::Unary(unary) => lower_unary_expression(unary, expression, ctx, self_type),
        _ => Err(FrontendError::UnsupportedExpression {
            location: location(expression),
        }),
    }
}

fn lower_method_call(
    call: &syn::ExprMethodCall,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    const TRANSPARENT_METHODS: &[&str] = &[
        "into",
        "unwrap",
        "clone",
        "as_str",
        "as_ref",
        "as_slice",
        "as_deref",
        "to_string",
        "to_owned",
    ];
    if TRANSPARENT_METHODS.contains(&call.method.to_string().as_str()) && call.args.is_empty() {
        return lower_expression(&call.receiver, ctx, self_type);
    }
    let receiver = lower_expression(&call.receiver, ctx, self_type)?;
    let args = call
        .args
        .iter()
        .map(|arg| lower_expression(arg, ctx, self_type))
        .collect::<FrontendResult<Vec<_>>>()?;
    Ok(factorio_ir::expression::Expression::MethodCall {
        receiver: Box::new(receiver),
        method: strip_raw_prefix(call.method.to_string()),
        args,
    })
}

fn lower_if_expr(
    if_expr: &syn::ExprIf,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let condition = lower_expression(&if_expr.cond, ctx, self_type)?;
    let else_branch =
        if_expr
            .else_branch
            .as_ref()
            .ok_or_else(|| FrontendError::UnsupportedExpression {
                location: location(if_expr),
            })?;
    let then_val = match if_expr.then_branch.stmts.as_slice() {
        [syn::Stmt::Expr(e, None)] => lower_expression(e, ctx, self_type)?,
        _ => {
            return Err(FrontendError::UnsupportedExpression {
                location: location(if_expr),
            });
        }
    };
    let else_val = match else_branch.1.as_ref() {
        Expr::Block(b) => match b.block.stmts.as_slice() {
            [syn::Stmt::Expr(e, None)] => lower_expression(e, ctx, self_type)?,
            _ => {
                return Err(FrontendError::UnsupportedExpression {
                    location: location(if_expr),
                });
            }
        },
        e => lower_expression(e, ctx, self_type)?,
    };
    // `cond and then_val or else_val`
    let and_part = factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(condition),
        op: factorio_ir::operator::Operator::And,
        rhs: Box::new(then_val),
    };
    Ok(factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(and_part),
        op: factorio_ir::operator::Operator::Or,
        rhs: Box::new(else_val),
    })
}

fn lower_unary_expression(
    unary: &syn::ExprUnary,
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    match unary.op {
        UnOp::Not(_) => {
            let inner = lower_expression(&unary.expr, ctx, self_type)?;
            Ok(factorio_ir::expression::Expression::Not(Box::new(inner)))
        }
        UnOp::Neg(_) => {
            // `-x` -> `0 - x`
            let inner = lower_expression(&unary.expr, ctx, self_type)?;
            Ok(factorio_ir::expression::Expression::BinaryOp {
                lhs: Box::new(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Int(0),
                )),
                op: factorio_ir::operator::Operator::Sub,
                rhs: Box::new(inner),
            })
        }
        // `*x` - dereference is a no-op in Lua; lower the inner expression directly.
        UnOp::Deref(_) => lower_expression(&unary.expr, ctx, self_type),
        _ => Err(FrontendError::UnsupportedExpression {
            location: location(expression),
        }),
    }
}

pub fn lower_assignment_target(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    match expression {
        Expr::Path(path) => lower_path_expression(path, ctx, self_type),
        Expr::Field(field) => lower_field_expression(field, ctx, self_type),
        _ => Err(FrontendError::ExpectedIdentifierAssignmentTarget {
            location: location(expression),
        }),
    }
}

fn lower_struct_expression(
    item: &syn::ExprStruct,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let struct_name = item.path.segments.last().map(|s| s.ident.to_string());

    let fields = item
        .fields
        .iter()
        .map(|field| {
            let name = match &field.member {
                Member::Named(ident) => ident.to_string(),
                Member::Unnamed(index) => {
                    return Err(FrontendError::UnsupportedExpression {
                        location: location(index),
                    });
                }
            };
            Ok((name, lower_expression(&field.expr, ctx, self_type)?))
        })
        .collect::<FrontendResult<Vec<_>>>()?;

    Ok(factorio_ir::expression::Expression::StructLiteral {
        struct_name,
        fields,
    })
}

fn lower_field_expression(
    field: &syn::ExprField,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let base = lower_expression(&field.base, ctx, self_type)?;
    let field_name = match &field.member {
        Member::Named(ident) => ident.to_string(),
        Member::Unnamed(index) => {
            return Err(FrontendError::UnsupportedExpression {
                location: location(index),
            });
        }
    };

    Ok(factorio_ir::expression::Expression::FieldAccess {
        base: Box::new(base),
        field: field_name,
    })
}

fn lower_binary_expression(
    binary: &ExprBinary,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let lhs = lower_expression(&binary.left, ctx, self_type)?;
    let op = lower_binary_operator(&binary.op)?;
    let rhs = lower_expression(&binary.right, ctx, self_type)?;

    Ok(factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(lhs),
        op,
        rhs: Box::new(rhs),
    })
}

fn lower_binary_operator(operator: &BinOp) -> FrontendResult<factorio_ir::operator::Operator> {
    let operator = match operator {
        BinOp::Add(_) => factorio_ir::operator::Operator::Add,
        BinOp::Sub(_) => factorio_ir::operator::Operator::Sub,
        BinOp::Mul(_) => factorio_ir::operator::Operator::Mul,
        BinOp::Div(_) => factorio_ir::operator::Operator::Div,
        BinOp::Eq(_) => factorio_ir::operator::Operator::Eq,
        BinOp::Ne(_) => factorio_ir::operator::Operator::Ne,
        BinOp::Lt(_) => factorio_ir::operator::Operator::Lt,
        BinOp::Le(_) => factorio_ir::operator::Operator::Le,
        BinOp::Gt(_) => factorio_ir::operator::Operator::Gt,
        BinOp::Ge(_) => factorio_ir::operator::Operator::Ge,
        BinOp::And(_) => factorio_ir::operator::Operator::And,
        BinOp::Or(_) => factorio_ir::operator::Operator::Or,
        BinOp::Rem(_) => factorio_ir::operator::Operator::Mod,
        _ => {
            return Err(FrontendError::UnsupportedOperator {
                location: location(operator),
            });
        }
    };

    Ok(operator)
}

fn lower_literal_expression(
    literal: &ExprLit,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let literal = match &literal.lit {
        Lit::Int(value) => {
            let parsed = value
                .base10_parse::<i64>()
                .map_err(|error| FrontendError::Syn(format!("invalid integer literal: {error}")))?;
            factorio_ir::literal::Literal::Int(parsed)
        }
        Lit::Float(value) => {
            let parsed = value
                .base10_parse::<f64>()
                .map_err(|error| FrontendError::Syn(format!("invalid float literal: {error}")))?;
            factorio_ir::literal::Literal::Float(parsed)
        }
        Lit::Str(value) => factorio_ir::literal::Literal::String(value.value()),
        Lit::Bool(value) => factorio_ir::literal::Literal::Bool(value.value),
        _ => {
            return Err(FrontendError::UnsupportedExpression {
                location: location(literal),
            });
        }
    };

    Ok(factorio_ir::expression::Expression::Literal(literal))
}

fn lower_path_expression(
    path: &ExprPath,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let mut segments = lower_path_segments(path, self_type)?;
    ctx.normalize_crate_path(&mut segments)?;
    // Rewrite bare imported module names, e.g. `adjacent_blacklist::check`
    // -> `ms_adjacent_blacklist::check` when prefix is set.
    ctx.normalize_bare_import_path(&mut segments);

    // Map Rust Option/bool keywords to Lua literals.
    if segments.len() == 1 {
        match segments[0].as_str() {
            "None" => {
                return Ok(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Nil,
                ));
            }
            "true" | "false" => {
                unreachable!("bool literals are handled by lower_literal_expression")
            }
            _ => {}
        }
    }

    // `Alignment::Center` / `unions::GuiDirection::Horizontal` -> Factorio string literal.
    if let Some(literal) = literal_enum_path_str(&segments) {
        return Ok(factorio_ir::expression::Expression::Literal(
            factorio_ir::literal::Literal::String(literal.to_string()),
        ));
    }

    match segments.len() {
        1 => Ok(factorio_ir::expression::Expression::Identifier(
            segments[0].clone(),
        )),
        _ => Ok(factorio_ir::expression::Expression::QualifiedPath { segments }),
    }
}

/// Resolve a path ending in `Type::Variant` to a Factorio literal-union string.
fn literal_enum_path_str(segments: &[String]) -> Option<&'static str> {
    if segments.len() < 2 {
        return None;
    }
    let variant = segments.last()?.as_str();
    let type_name = segments.get(segments.len().checked_sub(2)?)?.as_str();
    factorio_api::literal_enum_variant_str(type_name, variant)
}

fn lower_path_segments(path: &ExprPath, self_type: Option<&str>) -> FrontendResult<Vec<String>> {
    path.path
        .segments
        .iter()
        .map(|segment| resolve_path_segment(&segment.ident, self_type))
        .collect()
}

fn resolve_path_segment(ident: &syn::Ident, self_type: Option<&str>) -> FrontendResult<String> {
    if ident == "Self" {
        return self_type
            .map(str::to_string)
            .ok_or_else(|| FrontendError::UnsupportedExpression {
                location: location(ident),
            });
    }

    Ok(strip_raw_prefix(ident.to_string()))
}

/// Strip the `r#` raw-identifier prefix that Rust uses to escape keywords.
/// In Lua there is no such prefix; `r#type` should become `type`.
fn strip_raw_prefix(ident: String) -> String {
    ident
        .strip_prefix("r#")
        .map(str::to_string)
        .unwrap_or(ident)
}
