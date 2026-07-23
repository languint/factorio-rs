use syn::parse::{Parse, ParseStream};
use syn::{Expr, ExprMacro, LitStr, Token};

use crate::error::{FrontendError, FrontendResult};

use super::{
    context::LowerContext, expressions::lower_expression, print::lower_format_template,
    util::location,
};

struct AssertInput {
    condition: Expr,
    message: Option<FormatMessage>,
}

struct AssertCmpInput {
    left: Expr,
    right: Expr,
    message: Option<FormatMessage>,
}

struct FormatMessage {
    format: LitStr,
    args: Vec<Expr>,
}

impl Parse for AssertInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let condition = input.parse::<Expr>()?;
        let message = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                None
            } else {
                Some(input.parse()?)
            }
        } else {
            None
        };
        Ok(Self { condition, message })
    }
}

impl Parse for AssertCmpInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let left = input.parse::<Expr>()?;
        input.parse::<Token![,]>()?;
        let right = input.parse::<Expr>()?;
        let message = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                None
            } else {
                Some(input.parse()?)
            }
        } else {
            None
        };
        Ok(Self {
            left,
            right,
            message,
        })
    }
}

impl Parse for FormatMessage {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let format = input.parse::<LitStr>()?;
        let mut args = Vec::new();
        while input.parse::<Token![,]>().is_ok() {
            if input.is_empty() {
                break;
            }
            args.push(input.parse::<Expr>()?);
        }
        Ok(Self { format, args })
    }
}

/// Returns true when `mac` is an assertion / panic / todo macro we know how to lower.
#[must_use]
pub fn is_assert_macro(mac: &syn::Macro) -> bool {
    matches!(
        macro_name(&mac.path).as_str(),
        "assert" | "assert_eq" | "assert_ne" | "panic" | "todo" | "unimplemented"
    )
}

/// Lower an assertion / panic macro to statements (`if not ... then error(...) end`).
pub fn lower_assert_macro_statements(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match macro_name(&mac.mac.path).as_str() {
        "assert" => lower_assert(mac, ctx, self_type),
        "assert_eq" => lower_assert_cmp(mac, ctx, self_type, true),
        "assert_ne" => lower_assert_cmp(mac, ctx, self_type, false),
        "panic" => lower_panic(mac, ctx, self_type),
        "todo" | "unimplemented" => lower_todo_macro(mac, ctx, self_type),
        other => Err(FrontendError::UnsupportedMacro {
            name: other.to_string(),
            location: location(mac),
        }),
    }
}

fn lower_todo_macro(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let name = macro_name(&mac.mac.path);
    ctx.emit_lint(
        factorio_ir::lint::LintId::TodoMacro,
        format!("`{name}!` is not a finished Factorio code path; it lowers to `error(...)`"),
        location(mac),
    )?;
    let message = if mac.mac.tokens.is_empty() {
        string_lit(&format!("not yet implemented (`{name}!`)"))
    } else if let Ok(message) = syn::parse2::<FormatMessage>(mac.mac.tokens.clone()) {
        lower_message(&message, mac, ctx, self_type)?
    } else if let Ok(lit) = syn::parse2::<LitStr>(mac.mac.tokens.clone()) {
        string_lit(&lit.value())
    } else {
        return Err(FrontendError::UnsupportedMacro {
            name,
            location: location(mac)
                .with_note("expected `todo!()` / `unimplemented!()` or a string message"),
        });
    };
    Ok(vec![factorio_ir::statement::Statement::Expr(error_call(
        message,
    ))])
}

fn lower_assert(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let input = syn::parse2::<AssertInput>(mac.mac.tokens.clone()).map_err(|error| {
        FrontendError::UnsupportedMacro {
            name: "assert".to_string(),
            location: location(mac).with_note(error.to_string()),
        }
    })?;
    let condition = lower_expression(&input.condition, ctx, self_type)?;
    let message = match input.message {
        Some(ref message) => lower_message(message, mac, ctx, self_type)?,
        None => string_lit("assertion failed"),
    };
    Ok(vec![fail_unless(condition, message)])
}

fn lower_assert_cmp(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
    eq: bool,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let name = if eq { "assert_eq" } else { "assert_ne" };
    let input = syn::parse2::<AssertCmpInput>(mac.mac.tokens.clone()).map_err(|error| {
        FrontendError::UnsupportedMacro {
            name: name.to_string(),
            location: location(mac).with_note(error.to_string()),
        }
    })?;
    let left = lower_expression(&input.left, ctx, self_type)?;
    let right = lower_expression(&input.right, ctx, self_type)?;

    let left_name = ctx.alloc_assert_tmp("left");
    let right_name = ctx.alloc_assert_tmp("right");

    let mut stmts = vec![
        factorio_ir::statement::Statement::VariableDecl {
            name: left_name.clone(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: left,
        },
        factorio_ir::statement::Statement::VariableDecl {
            name: right_name.clone(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: right,
        },
    ];

    let left_id = factorio_ir::expression::Expression::Identifier(left_name);
    let right_id = factorio_ir::expression::Expression::Identifier(right_name);
    let cmp = factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(left_id.clone()),
        op: if eq {
            factorio_ir::operator::Operator::Eq
        } else {
            factorio_ir::operator::Operator::Ne
        },
        rhs: Box::new(right_id.clone()),
    };

    let default_msg = if eq {
        "assertion `left == right` failed"
    } else {
        "assertion `left != right` failed"
    };
    let custom = match input.message {
        Some(ref message) => Some(lower_message(message, mac, ctx, self_type)?),
        None => None,
    };
    let message = format_cmp_message(default_msg, custom, left_id, right_id);
    stmts.push(fail_unless(cmp, message));
    Ok(stmts)
}

fn lower_panic(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let message = if mac.mac.tokens.is_empty() {
        string_lit("explicit panic")
    } else if let Ok(message) = syn::parse2::<FormatMessage>(mac.mac.tokens.clone()) {
        lower_message(&message, mac, ctx, self_type)?
    } else if let Ok(lit) = syn::parse2::<LitStr>(mac.mac.tokens.clone()) {
        string_lit(&lit.value())
    } else {
        return Err(FrontendError::UnsupportedMacro {
            name: "panic".to_string(),
            location: location(mac).with_note("expected `panic!()` or `panic!(\"...\")`"),
        });
    };
    Ok(vec![factorio_ir::statement::Statement::Expr(error_call(
        message,
    ))])
}

fn lower_message(
    message: &FormatMessage,
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let mut args = Vec::with_capacity(message.args.len());
    for arg in &message.args {
        args.push(lower_expression(arg, ctx, self_type)?);
    }
    lower_format_template(
        &message.format.value(),
        &args,
        location(mac),
        &message.format,
        ctx,
    )
}

fn fail_unless(
    condition: factorio_ir::expression::Expression,
    message: factorio_ir::expression::Expression,
) -> factorio_ir::statement::Statement {
    factorio_ir::statement::Statement::Conditional {
        condition: factorio_ir::expression::Expression::Not(Box::new(condition)),
        then_block: vec![factorio_ir::statement::Statement::Expr(error_call(message))],
        else_block: vec![],
    }
}

fn error_call(message: factorio_ir::expression::Expression) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::Call {
        func: Box::new(factorio_ir::expression::Expression::Identifier(
            "error".to_string(),
        )),
        args: vec![message],
    }
}

fn format_cmp_message(
    default: &str,
    custom: Option<factorio_ir::expression::Expression>,
    left: factorio_ir::expression::Expression,
    right: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    let header = custom.unwrap_or_else(|| string_lit(default));
    let left_line = factorio_ir::expression::Expression::FormatConcat {
        parts: vec![string_lit("\n  left: "), tostring_call(left)],
    };
    let right_line = factorio_ir::expression::Expression::FormatConcat {
        parts: vec![string_lit("\n right: "), tostring_call(right)],
    };
    factorio_ir::expression::Expression::FormatConcat {
        parts: vec![header, left_line, right_line],
    }
}

fn tostring_call(
    value: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::Call {
        func: Box::new(factorio_ir::expression::Expression::Identifier(
            "tostring".to_string(),
        )),
        args: vec![value],
    }
}

fn string_lit(value: &str) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::String(
        value.to_string(),
    ))
}

fn macro_name(path: &syn::Path) -> String {
    path.get_ident().map_or_else(
        || {
            path.segments
                .last()
                .map_or_else(|| "macro".to_string(), |segment| segment.ident.to_string())
        },
        ToString::to_string,
    )
}
