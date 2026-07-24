use syn::{Expr, ExprClosure, ItemFn, Pat, PatType, Signature};

use crate::error::{FrontendError, FrontendResult};

use super::{
    attrs::{parse_factorio_export_attribute, parse_factorio_inline_attribute},
    context::LowerContext,
    event_handler::resolve_event_handler,
    expressions::lower_expression,
    metadata::{extract_doc_comments, function_header_comment},
    statements::lower_block,
    types::{
        is_option_type, lower_binding_pattern, lower_type, receiver_source_string,
        return_type_string, rust_type_key, type_source_string,
    },
    util::location,
};

pub fn lower_function(
    function: &ItemFn,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<factorio_ir::function::Function> {
    lower_function_parts(function, ctx, None)
}

pub fn lower_impl_method(
    method: &syn::ImplItemFn,
    self_type: &str,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<factorio_ir::function::Function> {
    let function = ItemFn {
        attrs: method.attrs.clone(),
        vis: method.vis.clone(),
        sig: method.sig.clone(),
        block: Box::new(method.block.clone()),
    };
    lower_function_parts(&function, ctx, Some(self_type))
}

/// Lower `|params| body` / `|params| { ... }` to an anonymous Lua function value.
pub fn lower_closure(
    closure: &ExprClosure,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    if closure.asyncness.is_some() {
        return Err(FrontendError::UnsupportedExpression {
            location: location(closure).with_note("async closures are not supported"),
        });
    }

    let binding_snapshot = ctx.binding_types.clone();
    let option_snapshot = ctx.option_bindings.clone();
    let dyn_snapshot = ctx.dyn_locals.clone();
    let mut params = Vec::new();
    for pat in &closure.inputs {
        params.push(closure_param_name(pat, closure)?);
    }
    // Typed closure params: `|x: Result<...>|` / `|x: Option<...>|`
    for pat in &closure.inputs {
        if let Pat::Type(PatType { pat, ty, .. }) = pat {
            let name = closure_param_name(pat, closure)?;
            if let Some(key) = rust_type_key(ty, &ctx.type_aliases, &ctx.assoc_bindings) {
                ctx.bind_type(name.clone(), key);
            }
            if is_option_type(ty, &ctx.type_aliases, &ctx.assoc_bindings) {
                ctx.bind_option(name);
            }
        }
    }

    let body = match closure.body.as_ref() {
        Expr::Block(block) => lower_block(&block.block, ctx, self_type)?,
        expr => {
            let mark = ctx.try_hoist_mark();
            let value = lower_expression(expr, ctx, self_type)?;
            let mut statements = ctx.take_try_hoists_from(mark);
            statements.push(factorio_ir::statement::Statement::Return(Some(value)));
            factorio_ir::block::Block { statements }
        }
    };
    ctx.binding_types = binding_snapshot;
    ctx.option_bindings = option_snapshot;
    ctx.dyn_locals = dyn_snapshot;

    Ok(factorio_ir::expression::Expression::Closure { params, body })
}

fn closure_param_name(pat: &Pat, closure: &ExprClosure) -> FrontendResult<String> {
    match pat {
        Pat::Ident(ident) => Ok(ident.ident.to_string()),
        Pat::Type(PatType { pat, .. }) => closure_param_name(pat, closure),
        _ => Err(FrontendError::UnsupportedExpression {
            location: location(closure).with_note("closure parameters must be plain identifiers"),
        }),
    }
}

fn lower_function_parts(
    function: &ItemFn,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::function::Function> {
    let fn_name = function.sig.ident.to_string();
    let event_attr = resolve_event_handler(function).or_else(|| {
        ctx.meta_markers.events.get(&fn_name).map(|filter| {
            super::event_handler::ParsedEventHandler {
                event_name: fn_name.clone(),
                filter: filter.clone(),
            }
        })
    });
    let export = function
        .attrs
        .iter()
        .find_map(parse_factorio_export_attribute)
        .or_else(|| ctx.meta_markers.exports.get(&fn_name).cloned());
    let inline = function.attrs.iter().any(parse_factorio_inline_attribute)
        || ctx.meta_markers.inlines.contains(&fn_name);
    let export = if inline && export.is_none() {
        Some(factorio_ir::function::ExportMeta { interface: None })
    } else {
        export
    };
    let binding_snapshot = ctx.binding_types.clone();
    let option_snapshot = ctx.option_bindings.clone();
    let dyn_snapshot = ctx.dyn_locals.clone();
    let into_snapshot = ctx.into_params.clone();
    let return_into_snapshot = ctx.return_into;
    let in_unsafe_snapshot = ctx.in_unsafe;
    ctx.return_into = matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if super::convert::into_target_type(ty).is_some());
    if function.sig.unsafety.is_some() {
        ctx.in_unsafe = true;
    }
    let params = lower_parameters(&function.sig, ctx)?;
    let body = lower_block(&function.block, ctx, self_type)?;
    ctx.binding_types = binding_snapshot;
    ctx.option_bindings = option_snapshot;
    ctx.dyn_locals = dyn_snapshot;
    ctx.into_params = into_snapshot;
    ctx.return_into = return_into_snapshot;
    ctx.in_unsafe = in_unsafe_snapshot;
    Ok(factorio_ir::function::Function {
        name: function.sig.ident.to_string(),
        params,
        body,
        doc: extract_doc_comments(&function.attrs),
        debug: Some(factorio_ir::debug::FunctionDebug {
            header_comment: function_header_comment(
                &function.vis,
                &function.sig,
                &ctx.type_aliases,
                &ctx.assoc_bindings,
            ),
            return_type: return_type_string(&function.sig, &ctx.type_aliases, &ctx.assoc_bindings),
        }),
        event: event_attr.as_ref().map(|event| event.event_name.clone()),
        event_filter: event_attr.and_then(|event| event.filter),
        export,
        inline,
    })
}

fn lower_parameters(
    signature: &Signature,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<Vec<factorio_ir::function::Parameter>> {
    signature
        .inputs
        .iter()
        .map(|input| lower_parameter(input, ctx))
        .collect::<FrontendResult<Vec<_>>>()
}

fn lower_parameter(
    input: &syn::FnArg,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<factorio_ir::function::Parameter> {
    match input {
        syn::FnArg::Receiver(receiver) => Ok(factorio_ir::function::Parameter {
            name: "self".to_string(),
            r#type: factorio_ir::r#type::Type::Void,
            source_type: Some(receiver_source_string(receiver)),
        }),
        syn::FnArg::Typed(PatType { pat, ty, .. }) => {
            let name = lower_binding_pattern(pat)?;
            let r#type = lower_type(ty, &ctx.type_aliases, &ctx.assoc_bindings)?;
            if let Some(key) = rust_type_key(ty, &ctx.type_aliases, &ctx.assoc_bindings) {
                ctx.bind_type(name.clone(), key);
            }
            if is_option_type(ty, &ctx.type_aliases, &ctx.assoc_bindings) {
                ctx.bind_option(name.clone());
            }
            if super::convert::into_target_type(ty).is_some() {
                super::convert::bind_into_param(ctx, name.clone());
            }
            if let Some(trait_name) = super::traits::dyn_trait_name(ty) {
                ctx.bind_dyn(
                    name.clone(),
                    super::traits::dyn_local(trait_name, "Unknown"),
                );
            }

            Ok(factorio_ir::function::Parameter {
                name,
                r#type,
                source_type: Some(type_source_string(
                    ty,
                    &ctx.type_aliases,
                    &ctx.assoc_bindings,
                )),
            })
        }
    }
}
