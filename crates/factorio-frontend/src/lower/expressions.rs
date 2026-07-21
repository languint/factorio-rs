use syn::{BinOp, Expr, ExprBinary, ExprLit, ExprPath, Lit, Member, UnOp};

use crate::error::{FrontendError, FrontendResult};

use super::{
    context::LowerContext, functions::lower_closure, print::lower_macro_expression,
    serde_json::serde_json_path_name, test_steps::is_factorio_rs_test_steps, util::location,
};

#[cfg(feature = "serde")]
use super::serde_json::{
    classify_serde_json_fn, lower_serde_json_fn, unsupported_serde_json_fn_error,
};

/// Option/Result methods that share names; need a typed local to pick the right lower.
const OVERLAPPING_OPTION_RESULT_METHODS: &[&str] =
    &["map", "and_then", "unwrap_or", "unwrap_or_else", "or_else"];

#[allow(clippy::missing_const_for_fn)]
fn expr_kind_name(expression: &Expr) -> &'static str {
    match expression {
        Expr::Array(_) => "Array",
        Expr::Assign(_) => "Assign",
        Expr::Async(_) => "Async",
        Expr::Await(_) => "Await",
        Expr::Binary(_) => "Binary",
        Expr::Block(_) => "Block",
        Expr::Break(_) => "Break",
        Expr::Call(_) => "Call",
        Expr::Cast(_) => "Cast",
        Expr::Closure(_) => "Closure",
        Expr::Const(_) => "Const",
        Expr::Continue(_) => "Continue",
        Expr::Field(_) => "Field",
        Expr::ForLoop(_) => "ForLoop",
        Expr::Group(_) => "Group",
        Expr::If(_) => "If",
        Expr::Index(_) => "Index",
        Expr::Infer(_) => "Infer",
        Expr::Let(_) => "Let",
        Expr::Lit(_) => "Lit",
        Expr::Loop(_) => "Loop",
        Expr::Macro(_) => "Macro",
        Expr::Match(_) => "Match",
        Expr::MethodCall(_) => "MethodCall",
        Expr::Paren(_) => "Paren",
        Expr::Path(_) => "Path",
        Expr::Range(_) => "Range",
        Expr::RawAddr(_) => "RawAddr",
        Expr::Reference(_) => "Reference",
        Expr::Repeat(_) => "Repeat",
        Expr::Return(_) => "Return",
        Expr::Struct(_) => "Struct",
        Expr::Try(_) => "Try",
        Expr::TryBlock(_) => "TryBlock",
        Expr::Tuple(_) => "Tuple",
        Expr::Unary(_) => "Unary",
        Expr::Unsafe(_) => "Unsafe",
        Expr::Verbatim(_) => "Verbatim",
        Expr::While(_) => "While",
        Expr::Yield(_) => "Yield",
        _ => "Other",
    }
}

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
        Expr::Call(call) => lower_call_expression(call, ctx, self_type),
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
            // Int literals are shifted (`0` -> `1`). String (and other) literals are
            // unshifted dictionary keys. Only non-literal indexes need the lint.
            if !matches!(
                key,
                factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Int(_)
                        | factorio_ir::literal::Literal::String(_)
                        | factorio_ir::literal::Literal::Float(_)
                        | factorio_ir::literal::Literal::Bool(_)
                )
            ) {
                ctx.emit_lint(
                    factorio_ir::lint::LintId::VariableIndex,
                    "non-literal index is not shifted for Lua's 1-based tables (literals are `n -> n+1`; variables are passed through)",
                    location(index),
                )?;
            }
            Ok(factorio_ir::expression::Expression::Index {
                base: Box::new(base),
                key: Box::new(key),
            })
        }
        Expr::Reference(reference) => lower_expression(&reference.expr, ctx, self_type),
        // `x as T`.
        Expr::Cast(cast) => lower_cast_expression(cast, ctx, self_type),
        // `(expr)` - transparent grouping.
        Expr::Paren(paren) => lower_expression(&paren.expr, ctx, self_type),
        // `if cond { a } else { b }` as an expression -> safe Lua if/else (not `and`/`or`).
        Expr::If(if_expr) => lower_if_expr(if_expr, ctx, self_type),
        Expr::Match(match_expr) => {
            super::statements::lower_match_expression(match_expr, ctx, self_type)
        }
        Expr::Unary(unary) => lower_unary_expression(unary, expression, ctx, self_type),
        Expr::Closure(closure) => lower_closure(closure, ctx, self_type),
        Expr::Try(try_expr) => lower_try_expression(try_expr, ctx, self_type),
        Expr::Range(range) => Err(FrontendError::UnsupportedExpression {
            location: location(range)
                .with_note("use `for i in start..end` or `(start..end).map(...).collect()`"),
        }),
        other => Err(FrontendError::UnsupportedExpression {
            location: location(expression).with_note(expr_kind_name(other)),
        }),
    }
}

/// Lower `expr?`: Result (`.err` early-return) or Option (`nil` early-return).
///
/// Typed `Option` bindings use nil early-return. Everything else (including
/// call results) uses Result semantics. Untyped local bindings get
/// [`LintId::AmbiguousTry`]. Call/method `?` gets [`LintId::OptionTry`] because
/// Factorio APIs often return `Option` while lowering still assumes `Result`.
fn lower_try_expression(
    try_expr: &syn::ExprTry,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let option_try = expr_type_key(&try_expr.expr, ctx).is_some_and(|key| key == "Option");
    if is_untyped_local_path(&try_expr.expr, ctx) {
        ctx.emit_lint(
            factorio_ir::lint::LintId::AmbiguousTry,
            "`?` on an untyped local assumes `Result` (`.err` / `.ok`); annotate as `Result`/`Option` or use `.ok_or(...)?`",
            location(try_expr),
        )?;
    } else if is_call_like_try_expr(&try_expr.expr) && !option_try {
        ctx.emit_lint(
            factorio_ir::lint::LintId::OptionTry,
            "`?` on a call/method assumes `Result` (`.err` / `.ok`); Option APIs need a typed binding or `.ok_or(...)?`",
            location(try_expr),
        )?;
    }

    let inner = lower_expression(&try_expr.expr, ctx, self_type)?;
    let tmp = ctx.alloc_try_tmp();
    ctx.try_hoists
        .push(factorio_ir::statement::Statement::VariableDecl {
            name: tmp.clone(),
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value: inner,
        });

    if option_try {
        ctx.try_hoists
            .push(factorio_ir::statement::Statement::Conditional {
                condition: eq_nil(factorio_ir::expression::Expression::Identifier(tmp.clone())),
                then_block: vec![factorio_ir::statement::Statement::Return(Some(
                    factorio_ir::expression::Expression::Literal(
                        factorio_ir::literal::Literal::Nil,
                    ),
                ))],
                else_block: vec![],
            });
        return Ok(factorio_ir::expression::Expression::Identifier(tmp));
    }

    ctx.try_hoists
        .push(factorio_ir::statement::Statement::Conditional {
            condition: ne_nil(factorio_ir::expression::Expression::FieldAccess {
                base: Box::new(factorio_ir::expression::Expression::Identifier(tmp.clone())),
                field: "err".to_string(),
            }),
            then_block: vec![factorio_ir::statement::Statement::Return(Some(
                factorio_ir::expression::Expression::Identifier(tmp.clone()),
            ))],
            else_block: vec![],
        });
    Ok(factorio_ir::expression::Expression::FieldAccess {
        base: Box::new(factorio_ir::expression::Expression::Identifier(tmp)),
        field: "ok".to_string(),
    })
}

fn expr_type_key<'a>(expr: &'a Expr, ctx: &'a LowerContext<'_>) -> Option<&'a str> {
    match expr {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            ctx.binding_surface_type(&path.path.segments[0].ident.to_string())
        }
        Expr::Paren(paren) => expr_type_key(&paren.expr, ctx),
        Expr::Reference(reference) => expr_type_key(&reference.expr, ctx),
        Expr::Try(try_expr) => expr_type_key(&try_expr.expr, ctx),
        _ => None,
    }
}

/// A bare local path with no `Option`/`Result` surface type.
fn is_untyped_local_path(expr: &Expr, ctx: &LowerContext<'_>) -> bool {
    match expr {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            let name = path.path.segments[0].ident.to_string();
            ctx.binding_surface_type(&name).is_none()
        }
        Expr::Paren(paren) => is_untyped_local_path(&paren.expr, ctx),
        Expr::Reference(reference) => is_untyped_local_path(&reference.expr, ctx),
        _ => false,
    }
}

/// `foo()` / `recv.method(...)` (parens/refs peeled) - call-result `?` site.
fn is_call_like_try_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Call(_) | Expr::MethodCall(_) => true,
        Expr::Paren(paren) => is_call_like_try_expr(&paren.expr),
        Expr::Reference(reference) => is_call_like_try_expr(&reference.expr),
        Expr::Group(group) => is_call_like_try_expr(&group.expr),
        _ => false,
    }
}

#[allow(clippy::too_many_lines)]
fn lower_call_expression(
    call: &syn::ExprCall,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    // `Some(x)` / `Option::Some(x)` -> `x` (Option is value-or-nil in Lua).
    if is_option_some_constructor(&call.func) {
        let mut args = call.args.iter();
        let Some(arg) = args.next() else {
            return Err(FrontendError::UnsupportedExpression {
                location: location(call).with_note("Some expects exactly one argument"),
            });
        };
        if args.next().is_some() {
            return Err(FrontendError::UnsupportedExpression {
                location: location(call).with_note("Some expects exactly one argument"),
            });
        }
        return lower_expression(arg, ctx, self_type);
    }

    // `Ok(x)` / `Result::Ok(x)` -> `{ ok = x }`; `Err(e)` / `Result::Err(e)` -> `{ err = e }`.
    if let Some(kind) = result_constructor_kind(&call.func) {
        return lower_result_constructor(call, kind, ctx, self_type);
    }

    if let Expr::Path(path) = call.func.as_ref() {
        let segments = lower_path_segments(path, self_type)?;
        if let Some((enum_name, variant, super::context::EnumVariantFields::Tuple(count))) =
            enum_variant_from_segments(&segments, ctx)
        {
            if call.args.len() != count {
                return Err(FrontendError::UnsupportedExpression {
                    location: location(call)
                        .with_note(format!("{enum_name}::{variant} expects {count} arguments")),
                });
            }
            let fields = call
                .args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    Ok((
                        format!("_{}", index + 1),
                        lower_expression(arg, ctx, self_type)?,
                    ))
                })
                .collect::<FrontendResult<Vec<_>>>()?;
            return Ok(factorio_ir::expression::Expression::EnumLiteral {
                enum_name,
                variant,
                fields,
            });
        }
    }

    // `lua_fn(handler)` / `lua_fn0` / `lua_fn2` - stub-only coercion helpers; emit the fn name.
    if is_lua_fn_helper(&call.func) {
        let mut args = call.args.iter();
        let Some(arg) = args.next() else {
            return Err(FrontendError::UnsupportedExpression {
                location: location(call).with_note("lua_fn expects exactly one argument"),
            });
        };
        if args.next().is_some() {
            return Err(FrontendError::UnsupportedExpression {
                location: location(call).with_note("lua_fn expects exactly one argument"),
            });
        }
        return lower_expression(arg, ctx, self_type);
    }

    if let Some(func_name) = serde_json_path_name(&call.func) {
        #[cfg(not(feature = "serde"))]
        {
            let _ = func_name;
            return Err(FrontendError::UnsupportedExpression {
                location: location(call).with_note(
                    "serde_json lowering requires the `serde` feature on \
                     factorio-rs-cli / factorio-frontend",
                ),
            });
        }
        #[cfg(feature = "serde")]
        {
            let Some(kind) = classify_serde_json_fn(&func_name) else {
                return Err(unsupported_serde_json_fn_error(&func_name, location(call)));
            };
            let mut args = call.args.iter();
            let Some(arg) = args.next() else {
                return Err(FrontendError::UnsupportedExpression {
                    location: location(call).with_note(format!(
                        "serde_json::{func_name} expects exactly one argument"
                    )),
                });
            };
            if args.next().is_some() {
                return Err(FrontendError::UnsupportedExpression {
                    location: location(call).with_note(format!(
                        "serde_json::{func_name} expects exactly one argument"
                    )),
                });
            }
            let value = lower_expression(arg, ctx, self_type)?;
            return Ok(lower_serde_json_fn(kind, value));
        }
    }

    // `factorio_rs::test::steps()` -> `__frs_steps()` (harness intrinsic).
    if is_factorio_rs_test_steps(&call.func) {
        if !call.args.is_empty() {
            return Err(FrontendError::UnsupportedExpression {
                location: location(call).with_note("factorio_rs::test::steps() takes no arguments"),
            });
        }
        return Ok(factorio_ir::expression::Expression::Call {
            func: Box::new(factorio_ir::expression::Expression::Identifier(
                "__frs_steps".to_string(),
            )),
            args: vec![],
        });
    }

    if identification_ctor_type(&call.func).is_some() {
        if call.args.len() != 1 {
            return Err(FrontendError::UnsupportedExpression {
                location: location(call)
                    .with_note("Identification constructors take exactly one payload argument"),
            });
        }
        return lower_expression(&call.args[0], ctx, self_type);
    }

    if let Expr::Path(path) = call.func.as_ref() {
        let mut segments = lower_path_segments(path, self_type)?;
        if let Some((interface, fn_name)) = ctx.resolve_remote_call(&segments) {
            let mut args = vec![
                factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::String(interface),
                ),
                factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::String(fn_name),
                ),
            ];
            for arg in &call.args {
                args.push(lower_expression(arg, ctx, self_type)?);
            }
            return Ok(factorio_ir::expression::Expression::Call {
                func: Box::new(factorio_ir::expression::Expression::QualifiedPath {
                    segments: vec!["remote".to_string(), "call".to_string()],
                }),
                args,
            });
        }
        // Rewrite crate/binding paths with last-segment-as-fn for lowercase callees.
        ctx.normalize_crate_path_for_call(&mut segments)?;
        ctx.normalize_bare_import_path(&mut segments);
        if let Some((interface, fn_name)) = ctx.resolve_remote_call(&segments) {
            let mut args = vec![
                factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::String(interface),
                ),
                factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::String(fn_name),
                ),
            ];
            for arg in &call.args {
                args.push(lower_expression(arg, ctx, self_type)?);
            }
            return Ok(factorio_ir::expression::Expression::Call {
                func: Box::new(factorio_ir::expression::Expression::QualifiedPath {
                    segments: vec!["remote".to_string(), "call".to_string()],
                }),
                args,
            });
        }
        let func = match segments.len() {
            1 => factorio_ir::expression::Expression::Identifier(segments[0].clone()),
            _ => factorio_ir::expression::Expression::QualifiedPath { segments },
        };
        let dyn_params = match &func {
            factorio_ir::expression::Expression::Identifier(name) => {
                ctx.dyn_fn_params.get(name).cloned()
            }
            _ => None,
        };
        let args =
            lower_call_args_with_dyn_coerce(&call.args, dyn_params.as_deref(), ctx, self_type)?;
        return Ok(factorio_ir::expression::Expression::Call {
            func: Box::new(func),
            args,
        });
    }

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

/// `ForceID::Name(...)` / `concepts::ForceID::Name(...)` -> type name when Identification.
fn identification_ctor_type(func: &Expr) -> Option<String> {
    let Expr::Path(path) = func else {
        return None;
    };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    if segments.len() < 2 {
        return None;
    }
    let type_name = segments[segments.len() - 2].as_str();
    if factorio_api::debug_types::is_payload_ctor_type(type_name) {
        Some(type_name.to_string())
    } else {
        None
    }
}

fn is_option_some_constructor(func: &Expr) -> bool {
    let Expr::Path(path) = func else {
        return false;
    };
    let segments: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match segments.as_slice() {
        [name] if name == "Some" => true,
        [.., option, name] if option == "Option" && name == "Some" => true,
        _ => false,
    }
}

/// Returns `"Ok"` or `"Err"` when `func` is a Result constructor path.
fn result_constructor_kind(func: &Expr) -> Option<&'static str> {
    let Expr::Path(path) = func else {
        return None;
    };
    let segments: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match segments.as_slice() {
        [name] if name == "Ok" => Some("Ok"),
        [name] if name == "Err" => Some("Err"),
        [.., result, name] if result == "Result" && name == "Ok" => Some("Ok"),
        [.., result, name] if result == "Result" && name == "Err" => Some("Err"),
        _ => None,
    }
}

fn lower_result_constructor(
    call: &syn::ExprCall,
    kind: &str,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let mut args = call.args.iter();
    let Some(arg) = args.next() else {
        return Err(FrontendError::UnsupportedExpression {
            location: location(call).with_note(format!("{kind} expects exactly one argument")),
        });
    };
    if args.next().is_some() {
        return Err(FrontendError::UnsupportedExpression {
            location: location(call).with_note(format!("{kind} expects exactly one argument")),
        });
    }
    let value = lower_expression(arg, ctx, self_type)?;
    if kind == "Err" && is_nil_like_expression(arg, &value) {
        ctx.emit_lint(
            factorio_ir::lint::LintId::ErrNil,
            "`Err(nil)` / `Err(None)` is ambiguous with Ok (`r.err == nil`); use a non-nil error payload",
            location(arg),
        )?;
    }
    let field = match kind {
        "Ok" => "ok",
        "Err" => "err",
        _ => unreachable!(),
    };
    Ok(factorio_ir::expression::Expression::StructLiteral {
        struct_name: Some("Result".to_string()),
        fields: vec![(field.to_string(), value)],
    })
}

fn is_nil_like_expression(arg: &Expr, lowered: &factorio_ir::expression::Expression) -> bool {
    if matches!(
        lowered,
        factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::Nil)
    ) {
        return true;
    }
    match arg {
        Expr::Path(path) => {
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            match segments.as_slice() {
                [name] if name == "None" => true,
                [.., option, name] if option == "Option" && name == "None" => true,
                _ => false,
            }
        }
        Expr::Paren(paren) => is_nil_like_expression(&paren.expr, lowered),
        _ => false,
    }
}

fn is_lua_fn_helper(func: &Expr) -> bool {
    let Expr::Path(path) = func else {
        return false;
    };
    path.path.segments.last().is_some_and(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "lua_fn" | "lua_fn0" | "lua_fn2"
        )
    })
}

fn lower_method_call(
    call: &syn::ExprMethodCall,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    const TRANSPARENT_METHODS: &[&str] = &[
        "into",
        "clone",
        "as_str",
        "as_ref",
        "as_slice",
        "as_deref",
        "to_string",
        "to_owned",
    ];
    let method = call.method.to_string();

    if let Some(expression) = super::iterators::try_lower_iterator_chain(call, ctx, self_type)? {
        return Ok(expression);
    }
    if matches!(method.as_str(), "iter" | "into_iter") && call.args.is_empty() {
        return Err(FrontendError::UnsupportedExpression {
            location: location(call).with_note(
                "`.iter()` is only supported in `for x in v.iter()` or `v.iter().map(...).collect()`",
            ),
        });
    }

    if let Some(expr) = lower_result_option_methods(call, &method, ctx, self_type)? {
        return Ok(expr);
    }

    if TRANSPARENT_METHODS.contains(&method.as_str()) && call.args.is_empty() {
        return lower_expression(&call.receiver, ctx, self_type);
    }

    // Result methods that do not overlap Option names (safe without type info).
    if matches!(method.as_str(), "is_ok" | "is_err" | "map_err")
        && let Some(expr) = lower_result_method(call, ctx, self_type)?
    {
        return Ok(expr);
    }

    if OVERLAPPING_OPTION_RESULT_METHODS.contains(&method.as_str())
        && is_untyped_local_path(&call.receiver, ctx)
    {
        ctx.emit_lint(
            factorio_ir::lint::LintId::AmbiguousMethod,
            format!(
                "`.{method}()` on an untyped local could be Option or Result; annotate the binding"
            ),
            location(call),
        )?;
    }

    if let Some(expr) = lower_option_method(call, ctx, self_type)? {
        return Ok(expr);
    }

    // Dyn fat-pointer method dispatch.
    if dyn_receiver_local(&call.receiver, ctx).is_some() {
        let receiver = lower_expression(&call.receiver, ctx, self_type)?;
        let args = call
            .args
            .iter()
            .map(|arg| lower_expression(arg, ctx, self_type))
            .collect::<FrontendResult<Vec<_>>>()?;
        return Ok(factorio_ir::expression::Expression::DynMethodCall {
            receiver: Box::new(receiver),
            method: lua_method_name(&method),
            args,
        });
    }

    let receiver = lower_expression(&call.receiver, ctx, self_type)?;
    let args = call
        .args
        .iter()
        .map(|arg| lower_expression(arg, ctx, self_type))
        .collect::<FrontendResult<Vec<_>>>()?;
    let method_name = lua_method_name(&method);

    // User struct / trait methods: emit `Type.method(receiver, args...)` so Lua
    // receives `self`.
    if let Some(owner) = user_method_owner(&call.receiver, ctx, self_type) {
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(receiver);
        call_args.extend(args);
        return Ok(factorio_ir::expression::Expression::Call {
            func: Box::new(factorio_ir::expression::Expression::QualifiedPath {
                segments: vec![owner, method_name],
            }),
            args: call_args,
        });
    }

    Ok(factorio_ir::expression::Expression::MethodCall {
        receiver: Box::new(receiver),
        method: method_name,
        args,
    })
}

fn user_method_owner(
    receiver: &Expr,
    ctx: &LowerContext<'_>,
    self_type: Option<&str>,
) -> Option<String> {
    let name = match receiver {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            path.path.segments[0].ident.to_string()
        }
        Expr::Struct(item) => {
            let name = item.path.segments.last()?.ident.to_string();
            return ctx.is_user_struct(&name).then_some(name);
        }
        Expr::Paren(paren) => return user_method_owner(&paren.expr, ctx, self_type),
        Expr::Group(group) => return user_method_owner(&group.expr, ctx, self_type),
        Expr::Reference(reference) => return user_method_owner(&reference.expr, ctx, self_type),
        _ => return None,
    };
    if name == "self" {
        let owner = self_type?;
        return ctx.is_user_struct(owner).then(|| owner.to_string());
    }
    let key = ctx.binding_type(&name)?;
    ctx.is_user_struct(key).then(|| key.to_string())
}

fn lower_result_option_methods(
    call: &syn::ExprMethodCall,
    method: &str,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Option<factorio_ir::expression::Expression>> {
    // Result-typed receivers: prefer Result helpers (including unwrap -> `.ok`).
    if receiver_is_result(&call.receiver, ctx) {
        if method == "unwrap" && call.args.is_empty() {
            ctx.emit_lint(
                factorio_ir::lint::LintId::Unwrap,
                "`.unwrap()` does not check for Err in Lua; use `if let Ok(...)` or `?` instead",
                location(call),
            )?;
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            return Ok(Some(result_ok_field(receiver)));
        }
        if method == "expect" && call.args.len() == 1 {
            ctx.emit_lint(
                factorio_ir::lint::LintId::Expect,
                "`.expect(...)` does not check for Err in Lua; the message is discarded",
                location(call),
            )?;
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            return Ok(Some(result_ok_field(receiver)));
        }
        if let Some(expr) = lower_result_method(call, ctx, self_type)? {
            return Ok(Some(expr));
        }
    }

    if method == "unwrap" && call.args.is_empty() {
        ctx.emit_lint(
            factorio_ir::lint::LintId::Unwrap,
            "`.unwrap()` does not check for nil in Lua; use `if let Some(...)` instead",
            location(call),
        )?;
        return Ok(Some(lower_expression(&call.receiver, ctx, self_type)?));
    }
    if method == "expect" && call.args.len() == 1 {
        ctx.emit_lint(
            factorio_ir::lint::LintId::Expect,
            "`.expect(...)` does not check for nil in Lua; the message is discarded",
            location(call),
        )?;
        return Ok(Some(lower_expression(&call.receiver, ctx, self_type)?));
    }
    Ok(None)
}

fn dyn_receiver_local<'a>(
    receiver: &Expr,
    ctx: &'a LowerContext<'_>,
) -> Option<&'a super::context::DynLocal> {
    match receiver {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            ctx.dyn_local(&path.path.segments[0].ident.to_string())
        }
        Expr::Paren(paren) => dyn_receiver_local(&paren.expr, ctx),
        Expr::Reference(reference) => dyn_receiver_local(&reference.expr, ctx),
        _ => None,
    }
}

fn lower_cast_expression(
    cast: &syn::ExprCast,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let Some(trait_name) = super::traits::dyn_trait_name(&cast.ty) else {
        return lower_expression(&cast.expr, ctx, self_type);
    };
    coerce_expr_to_dyn(&cast.expr, &trait_name, ctx, self_type, location(cast))
}

/// Lower call arguments, wrapping concrete values as fat pointers when the
/// callee parameter is `&dyn Trait` / `Box<dyn Trait>`.
fn lower_call_args_with_dyn_coerce(
    args: &syn::punctuated::Punctuated<Expr, syn::token::Comma>,
    dyn_params: Option<&[Option<String>]>,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::expression::Expression>> {
    args.iter()
        .enumerate()
        .map(|(index, arg)| {
            let expected = dyn_params
                .and_then(|params| params.get(index))
                .and_then(|t| t.as_deref());
            match expected {
                Some(trait_name) => lower_dyn_call_arg(arg, trait_name, ctx, self_type),
                None => lower_expression(arg, ctx, self_type),
            }
        })
        .collect()
}

fn lower_dyn_call_arg(
    arg: &Expr,
    trait_name: &str,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    // Explicit cast already produces a fat pointer.
    if let Expr::Cast(cast) = arg
        && super::traits::dyn_trait_name(&cast.ty).as_deref() == Some(trait_name)
    {
        return lower_expression(arg, ctx, self_type);
    }

    // Already a dyn local (optionally behind `&`): pass through.
    if dyn_receiver_local(arg, ctx).is_some_and(|d| d.trait_name == trait_name) {
        return lower_expression(arg, ctx, self_type);
    }

    coerce_expr_to_dyn(arg, trait_name, ctx, self_type, location(arg))
}

fn coerce_expr_to_dyn(
    expr: &Expr,
    trait_name: &str,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
    loc: factorio_ir::span::SourceLoc,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let trait_info = ctx.traits.get(trait_name).cloned().ok_or_else(|| {
        FrontendError::UnsupportedExpression {
            location: loc.clone().with_note(format!(
                "unknown trait `{trait_name}` in dyn coerce; define it in this module or `use` it from another module"
            )),
        }
    })?;
    super::traits::ensure_object_safe(&trait_info, loc.clone())?;

    let concrete_name = super::traits::resolve_concrete_type(expr, ctx).ok_or_else(|| {
        FrontendError::UnsupportedExpression {
            location: loc.with_note(
                "could not resolve concrete type for dyn coerce; use a struct literal, typed local, or `as &dyn Trait`",
            ),
        }
    })?;

    let vt = super::traits::vtable_name(trait_name, &concrete_name);
    if !ctx.vtables.iter().any(|vtable| vtable.name == vt) {
        ctx.vtables.push(factorio_ir::module::VTable {
            name: vt.clone(),
            concrete_type: concrete_name,
            methods: trait_info.methods.keys().cloned().collect(),
        });
    }

    let inner = super::traits::peel_box_new(expr);
    let data = lower_expression(inner, ctx, self_type)?;
    Ok(factorio_ir::expression::Expression::FatPointer {
        data: Box::new(data),
        vtable: vt,
    })
}

fn receiver_is_result(receiver: &Expr, ctx: &LowerContext<'_>) -> bool {
    expr_type_key(receiver, ctx).is_some_and(|key| key == "Result")
}

fn result_ok_field(
    receiver: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::FieldAccess {
        base: Box::new(receiver),
        field: "ok".to_string(),
    }
}

fn result_err_field(
    receiver: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::FieldAccess {
        base: Box::new(receiver),
        field: "err".to_string(),
    }
}

fn result_is_ok(
    receiver: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    eq_nil(result_err_field(receiver))
}

fn result_is_err(
    receiver: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    ne_nil(result_err_field(receiver))
}

fn result_ok_wrap(
    value: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::StructLiteral {
        struct_name: Some("Result".to_string()),
        fields: vec![("ok".to_string(), value)],
    }
}

fn result_err_wrap(
    value: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::StructLiteral {
        struct_name: Some("Result".to_string()),
        fields: vec![("err".to_string(), value)],
    }
}

const fn receiver_is_trivial(expr: &factorio_ir::expression::Expression) -> bool {
    matches!(
        expr,
        factorio_ir::expression::Expression::Identifier(_)
            | factorio_ir::expression::Expression::Literal(_)
    )
}

/// Evaluate `receiver` once when `build` may mention it more than once.
///
/// Non-trivial receivers (calls, field chains, ...) are bound to a local inside an
/// IIFE so side-effecting expressions are not duplicated.
fn with_receiver_once(
    receiver: factorio_ir::expression::Expression,
    build: impl FnOnce(factorio_ir::expression::Expression) -> factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    if receiver_is_trivial(&receiver) {
        return build(receiver);
    }

    let tmp = "__o".to_string();
    let bound = factorio_ir::expression::Expression::Identifier(tmp.clone());
    let value = build(bound);
    let mut statements = vec![factorio_ir::statement::Statement::VariableDecl {
        name: tmp,
        ty: factorio_ir::r#type::Type::Void,
        source_type: None,
        value: receiver,
    }];
    match value {
        factorio_ir::expression::Expression::If {
            condition,
            then_expr,
            else_expr,
        } => {
            statements.push(factorio_ir::statement::Statement::Conditional {
                condition: *condition,
                then_block: vec![factorio_ir::statement::Statement::Return(Some(*then_expr))],
                else_block: vec![factorio_ir::statement::Statement::Return(Some(*else_expr))],
            });
        }
        other => {
            statements.push(factorio_ir::statement::Statement::Return(Some(other)));
        }
    }

    factorio_ir::expression::Expression::Call {
        func: Box::new(factorio_ir::expression::Expression::Closure {
            params: vec![],
            body: factorio_ir::block::Block { statements },
        }),
        args: vec![],
    }
}

/// Result helpers. Returns `Ok(None)` when the method is not a Result special.
fn lower_result_method(
    call: &syn::ExprMethodCall,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Option<factorio_ir::expression::Expression>> {
    let method = call.method.to_string();
    match method.as_str() {
        "is_ok" if call.args.is_empty() => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            Ok(Some(result_is_ok(receiver)))
        }
        "is_err" if call.args.is_empty() => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            Ok(Some(result_is_err(receiver)))
        }
        "unwrap_or" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let default = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(result_is_ok(r.clone())),
                    then_expr: Box::new(result_ok_field(r)),
                    else_expr: Box::new(default),
                }
            })))
        }
        "map" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let func = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                let mapped = factorio_ir::expression::Expression::Call {
                    func: Box::new(func),
                    args: vec![result_ok_field(r.clone())],
                };
                factorio_ir::expression::Expression::If {
                    condition: Box::new(result_is_ok(r.clone())),
                    then_expr: Box::new(result_ok_wrap(mapped)),
                    else_expr: Box::new(r),
                }
            })))
        }
        "map_err" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let func = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                let mapped = factorio_ir::expression::Expression::Call {
                    func: Box::new(func),
                    args: vec![result_err_field(r.clone())],
                };
                factorio_ir::expression::Expression::If {
                    condition: Box::new(result_is_err(r.clone())),
                    then_expr: Box::new(result_err_wrap(mapped)),
                    else_expr: Box::new(r),
                }
            })))
        }
        "and_then" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let func = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(result_is_ok(r.clone())),
                    then_expr: Box::new(factorio_ir::expression::Expression::Call {
                        func: Box::new(func),
                        args: vec![result_ok_field(r.clone())],
                    }),
                    else_expr: Box::new(r),
                }
            })))
        }
        "or_else" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let func = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(result_is_ok(r.clone())),
                    then_expr: Box::new(r.clone()),
                    else_expr: Box::new(factorio_ir::expression::Expression::Call {
                        func: Box::new(func),
                        args: vec![result_err_field(r)],
                    }),
                }
            })))
        }
        "unwrap_or_else" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let func = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(result_is_ok(r.clone())),
                    then_expr: Box::new(result_ok_field(r.clone())),
                    else_expr: Box::new(factorio_ir::expression::Expression::Call {
                        func: Box::new(func),
                        args: vec![result_err_field(r)],
                    }),
                }
            })))
        }
        _ => Ok(None),
    }
}

/// Nil-aware Option helpers. Returns `Ok(None)` when the method is not an Option special.
#[allow(clippy::too_many_lines)]
fn lower_option_method(
    call: &syn::ExprMethodCall,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Option<factorio_ir::expression::Expression>> {
    let method = call.method.to_string();
    match method.as_str() {
        "is_some" if call.args.is_empty() => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            Ok(Some(ne_nil(receiver)))
        }
        "is_none" if call.args.is_empty() => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            Ok(Some(eq_nil(receiver)))
        }
        "unwrap_or" | "or" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let default = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(ne_nil(r.clone())),
                    then_expr: Box::new(r),
                    else_expr: Box::new(default),
                }
            })))
        }
        "and" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let other = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(factorio_ir::expression::Expression::If {
                condition: Box::new(ne_nil(receiver)),
                then_expr: Box::new(other),
                else_expr: Box::new(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Nil,
                )),
            }))
        }
        "map" | "and_then" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let func = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(ne_nil(r.clone())),
                    then_expr: Box::new(factorio_ir::expression::Expression::Call {
                        func: Box::new(func),
                        args: vec![r],
                    }),
                    else_expr: Box::new(factorio_ir::expression::Expression::Literal(
                        factorio_ir::literal::Literal::Nil,
                    )),
                }
            })))
        }
        "unwrap_or_else" | "or_else" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let func = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(ne_nil(r.clone())),
                    then_expr: Box::new(r),
                    else_expr: Box::new(factorio_ir::expression::Expression::Call {
                        func: Box::new(func),
                        args: vec![],
                    }),
                }
            })))
        }
        "filter" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let pred = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                let keep = factorio_ir::expression::Expression::If {
                    condition: Box::new(factorio_ir::expression::Expression::Call {
                        func: Box::new(pred),
                        args: vec![r.clone()],
                    }),
                    then_expr: Box::new(r.clone()),
                    else_expr: Box::new(factorio_ir::expression::Expression::Literal(
                        factorio_ir::literal::Literal::Nil,
                    )),
                };
                factorio_ir::expression::Expression::If {
                    condition: Box::new(ne_nil(r)),
                    then_expr: Box::new(keep),
                    else_expr: Box::new(factorio_ir::expression::Expression::Literal(
                        factorio_ir::literal::Literal::Nil,
                    )),
                }
            })))
        }
        // Option -> Result
        "ok_or" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let err = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(ne_nil(r.clone())),
                    then_expr: Box::new(result_ok_wrap(r)),
                    else_expr: Box::new(result_err_wrap(err)),
                }
            })))
        }
        "ok_or_else" if call.args.len() == 1 => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let func = lower_expression(&call.args[0], ctx, self_type)?;
            Ok(Some(with_receiver_once(receiver, |r| {
                factorio_ir::expression::Expression::If {
                    condition: Box::new(ne_nil(r.clone())),
                    then_expr: Box::new(result_ok_wrap(r)),
                    else_expr: Box::new(result_err_wrap(
                        factorio_ir::expression::Expression::Call {
                            func: Box::new(func),
                            args: vec![],
                        },
                    )),
                }
            })))
        }
        _ => Ok(None),
    }
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

fn eq_nil(expr: factorio_ir::expression::Expression) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(expr),
        op: factorio_ir::operator::Operator::Eq,
        rhs: Box::new(factorio_ir::expression::Expression::Literal(
            factorio_ir::literal::Literal::Nil,
        )),
    }
}

/// Remap Rust overload aliases to the real Lua library method name.
fn lua_method_name(method: &str) -> String {
    let name = strip_raw_prefix(method.to_string());
    match name.as_str() {
        "random_int" | "random_range" => "random".to_string(),
        "format_1" | "format_2" | "format_3" | "format_4" => "format".to_string(),
        "insert_at" => "insert".to_string(),
        _ => name,
    }
}

fn lower_if_expr(
    if_expr: &syn::ExprIf,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    match expr_type_key(&if_expr.cond, ctx) {
        Some("Option") => {
            ctx.emit_lint(
                factorio_ir::lint::LintId::OptionIf,
                "`if option { ... }` uses Lua truthiness (`Some(false)` / `Some(0)` are skipped); use `if let Some(...)` or `.is_some()`",
                location(&if_expr.cond),
            )?;
        }
        Some("Result") => {
            ctx.emit_lint(
                factorio_ir::lint::LintId::ResultIf,
                "`if result { ... }` is always truthy in Lua (Result is a table); use `if let Ok(...)` or `.is_ok()`",
                location(&if_expr.cond),
            )?;
        }
        _ => {}
    }
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
    Ok(factorio_ir::expression::Expression::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_val),
        else_expr: Box::new(else_val),
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
        Expr::Index(index) => {
            let base = lower_expression(&index.expr, ctx, self_type)?;
            let key = lower_expression(&index.index, ctx, self_type)?;
            if !matches!(
                key,
                factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::Int(_)
                        | factorio_ir::literal::Literal::String(_)
                        | factorio_ir::literal::Literal::Float(_)
                        | factorio_ir::literal::Literal::Bool(_)
                )
            ) {
                ctx.emit_lint(
                    factorio_ir::lint::LintId::VariableIndex,
                    "non-literal index is not shifted for Lua's 1-based tables (literals are `n -> n+1`; variables are passed through)",
                    location(index),
                )?;
            }
            Ok(factorio_ir::expression::Expression::Index {
                base: Box::new(base),
                key: Box::new(key),
            })
        }
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
    let segments = item
        .path
        .segments
        .iter()
        .map(|segment| resolve_path_segment(&segment.ident, self_type))
        .collect::<FrontendResult<Vec<_>>>()?;

    if let Some(rest) = &item.rest
        && !is_default_default_expr(rest)
    {
        ctx.emit_lint(
            factorio_ir::lint::LintId::StructRest,
            "struct update `..rest` other than `Default::default()` is ignored; only explicit fields are emitted",
            location(rest.as_ref()),
        )?;
    }

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

    if let Some((enum_name, variant, super::context::EnumVariantFields::Named)) =
        enum_variant_from_segments(&segments, ctx)
    {
        return Ok(factorio_ir::expression::Expression::EnumLiteral {
            enum_name,
            variant,
            fields,
        });
    }
    Ok(factorio_ir::expression::Expression::StructLiteral {
        struct_name: segments.last().cloned(),
        fields,
    })
}

/// `Default::default()` / `default()` - the intentional sparse-table rest form.
fn is_default_default_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Call(call) => match call.func.as_ref() {
            Expr::Path(path) if call.args.is_empty() => {
                let segments: Vec<_> = path
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect();
                let names: Vec<&str> = segments.iter().map(String::as_str).collect();
                matches!(
                    names.as_slice(),
                    ["Default", "default"] | ["default"] | [.., "Default", "default"]
                )
            }
            _ => false,
        },
        Expr::Paren(paren) => is_default_default_expr(&paren.expr),
        Expr::Group(group) => is_default_default_expr(&group.expr),
        _ => false,
    }
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
    if matches!(binary.op, BinOp::Div(_))
        && !div_operand_looks_float(&binary.left, ctx)
        && !div_operand_looks_float(&binary.right, ctx)
    {
        ctx.emit_lint(
            factorio_ir::lint::LintId::IntegerDiv,
            "division lowers to Lua `/` (always float); Rust integer `/` truncates",
            location(binary),
        )?;
    }

    let lhs = lower_expression(&binary.left, ctx, self_type)?;
    let op = lower_binary_operator(&binary.op)?;
    let rhs = lower_expression(&binary.right, ctx, self_type)?;

    Ok(factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(lhs),
        op,
        rhs: Box::new(rhs),
    })
}

pub(super) fn div_operand_looks_float(expr: &Expr, ctx: &LowerContext<'_>) -> bool {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Float(_), ..
        }) => true,
        Expr::Path(path) if path.path.segments.len() == 1 => {
            matches!(
                ctx.binding_type(&path.path.segments[0].ident.to_string()),
                Some("f32" | "f64")
            )
        }
        Expr::Paren(paren) => div_operand_looks_float(&paren.expr, ctx),
        Expr::Group(group) => div_operand_looks_float(&group.expr, ctx),
        Expr::Unary(unary) => div_operand_looks_float(&unary.expr, ctx),
        Expr::Reference(reference) => div_operand_looks_float(&reference.expr, ctx),
        Expr::Cast(cast) => type_looks_float(&cast.ty),
        _ => false,
    }
}

fn type_looks_float(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| matches!(segment.ident.to_string().as_str(), "f32" | "f64")),
        syn::Type::Reference(reference) => type_looks_float(&reference.elem),
        syn::Type::Paren(paren) => type_looks_float(&paren.elem),
        _ => false,
    }
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

    if let Some((enum_name, variant, super::context::EnumVariantFields::Unit)) =
        enum_variant_from_segments(&segments, ctx)
    {
        return Ok(factorio_ir::expression::Expression::EnumLiteral {
            enum_name,
            variant,
            fields: vec![],
        });
    }

    match segments.len() {
        1 => Ok(factorio_ir::expression::Expression::Identifier(
            segments[0].clone(),
        )),
        _ => Ok(factorio_ir::expression::Expression::QualifiedPath { segments }),
    }
}

fn enum_variant_from_segments(
    segments: &[String],
    ctx: &LowerContext<'_>,
) -> Option<(String, String, super::context::EnumVariantFields)> {
    let variant = segments.last()?.clone();
    let enum_name = segments.get(segments.len().checked_sub(2)?)?.clone();
    ctx.enum_variant(&enum_name, &variant)
        .map(|fields| (enum_name, variant, fields))
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
