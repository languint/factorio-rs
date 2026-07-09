use syn::{Attribute, Block, ItemFn, PatType, Signature, Visibility};

use crate::error::FrontendResult;

use super::{
    attrs::extract_factorio_event,
    context::LowerContext,
    metadata::{extract_doc_comments, function_header_comment},
    statements::lower_block,
    types::{lower_binding_pattern, lower_type, receiver_source_string, return_type_string, type_source_string},
};

pub fn lower_function(
    function: &ItemFn,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<factorio_ir::function::Function> {
    lower_function_parts(
        &function.sig,
        &function.block,
        &function.vis,
        &function.attrs,
        ctx,
        None,
    )
}

pub fn lower_impl_method(
    method: &syn::ImplItemFn,
    self_type: &str,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<factorio_ir::function::Function> {
    lower_function_parts(
        &method.sig,
        &method.block,
        &method.vis,
        &method.attrs,
        ctx,
        Some(self_type),
    )
}

fn lower_function_parts(
    signature: &Signature,
    block: &Block,
    visibility: &Visibility,
    attrs: &[Attribute],
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::function::Function> {
    Ok(factorio_ir::function::Function {
        name: signature.ident.to_string(),
        params: lower_parameters(signature)?,
        body: lower_block(block, ctx, self_type)?,
        doc: extract_doc_comments(attrs),
        debug: Some(factorio_ir::debug::FunctionDebug {
            header_comment: function_header_comment(visibility, signature),
            return_type: return_type_string(signature),
        }),
        event: extract_factorio_event(attrs),
    })
}

fn lower_parameters(
    signature: &Signature,
) -> FrontendResult<Vec<factorio_ir::function::Parameter>> {
    signature
        .inputs
        .iter()
        .map(lower_parameter)
        .collect::<FrontendResult<Vec<_>>>()
}

fn lower_parameter(input: &syn::FnArg) -> FrontendResult<factorio_ir::function::Parameter> {
    match input {
        syn::FnArg::Receiver(receiver) => Ok(factorio_ir::function::Parameter {
            name: "self".to_string(),
            r#type: factorio_ir::r#type::Type::Void,
            source_type: Some(receiver_source_string(receiver)),
        }),
        syn::FnArg::Typed(PatType { pat, ty, .. }) => {
            let name = lower_binding_pattern(pat)?;
            let r#type = lower_type(ty)?;

            Ok(factorio_ir::function::Parameter {
                name,
                r#type,
                source_type: Some(type_source_string(ty)),
            })
        }
    }
}
