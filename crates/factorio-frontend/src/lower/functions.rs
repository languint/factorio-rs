use syn::{Expr, ExprClosure, ItemFn, Pat, PatType, Signature};

use crate::error::{FrontendError, FrontendResult};

use super::{
    context::LowerContext,
    event_handler::resolve_event_handler,
    expressions::lower_expression,
    metadata::{extract_doc_comments, function_header_comment},
    statements::lower_block,
    types::{
        lower_binding_pattern, lower_type, receiver_source_string, return_type_string,
        rust_type_key, type_source_string,
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
    let mut params = Vec::new();
    for pat in &closure.inputs {
        params.push(closure_param_name(pat, closure)?);
    }

    let body = match closure.body.as_ref() {
        Expr::Block(block) => lower_block(&block.block, ctx, self_type)?,
        expr => {
            let value = lower_expression(expr, ctx, self_type)?;
            factorio_ir::block::Block {
                statements: vec![factorio_ir::statement::Statement::Return(Some(value))],
            }
        }
    };
    ctx.binding_types = binding_snapshot;

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
    let event_attr = resolve_event_handler(function);
    let binding_snapshot = ctx.binding_types.clone();
    let params = lower_parameters(&function.sig, ctx)?;
    let body = lower_block(&function.block, ctx, self_type)?;
    ctx.binding_types = binding_snapshot;
    Ok(factorio_ir::function::Function {
        name: function.sig.ident.to_string(),
        params,
        body,
        doc: extract_doc_comments(&function.attrs),
        debug: Some(factorio_ir::debug::FunctionDebug {
            header_comment: function_header_comment(&function.vis, &function.sig),
            return_type: return_type_string(&function.sig),
        }),
        event: event_attr.as_ref().map(|event| event.event_name.clone()),
        event_filter: event_attr.and_then(|event| event.filter),
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
            let r#type = lower_type(ty)?;
            if let Some(key) = rust_type_key(ty) {
                ctx.bind_type(name.clone(), key);
            }

            Ok(factorio_ir::function::Parameter {
                name,
                r#type,
                source_type: Some(type_source_string(ty)),
            })
        }
    }
}
