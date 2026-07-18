use syn::{Attribute, Expr, ExprLit, FnArg, Lit, Meta, Signature, Visibility};

use super::types::{
    TypeAlias, lower_binding_pattern, receiver_source_string, return_type_string, type_source_string,
};

const fn visibility_prefix(visibility: &Visibility) -> &'static str {
    if matches!(visibility, Visibility::Public(_)) {
        "pub "
    } else {
        ""
    }
}

pub fn extract_doc_comments(attrs: &[Attribute]) -> Option<String> {
    let docs = attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }

            let Meta::NameValue(meta) = &attr.meta else {
                return None;
            };

            let Expr::Lit(ExprLit {
                lit: Lit::Str(value),
                ..
            }) = &meta.value
            else {
                return None;
            };

            Some(value.value().trim().to_string())
        })
        .collect::<Vec<_>>();

    if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    }
}

fn format_fn_arg_for_comment(
    arg: &FnArg,
    aliases: &std::collections::HashMap<String, TypeAlias>,
) -> String {
    match arg {
        FnArg::Receiver(receiver) => receiver_source_string(receiver),
        FnArg::Typed(pat_type) => {
            let name = lower_binding_pattern(&pat_type.pat).unwrap_or_else(|_| "_".to_string());
            format!("{}: {}", name, type_source_string(&pat_type.ty, aliases))
        }
    }
}

pub fn function_header_comment(
    visibility: &Visibility,
    signature: &Signature,
    aliases: &std::collections::HashMap<String, TypeAlias>,
) -> String {
    let params = signature
        .inputs
        .iter()
        .map(|arg| format_fn_arg_for_comment(arg, aliases))
        .collect::<Vec<_>>()
        .join(", ");
    let return_suffix = return_type_string(signature, aliases)
        .map_or_else(String::new, |return_type| format!(" -> {return_type}"));

    format!(
        "{}fn {}({}){}",
        visibility_prefix(visibility),
        signature.ident,
        params,
        return_suffix
    )
}

pub fn struct_header_comment(
    visibility: &Visibility,
    name: &str,
    fields: &[factorio_ir::structure::StructField],
) -> String {
    if fields.is_empty() {
        return format!("{}struct {name}", visibility_prefix(visibility));
    }

    let field_list = fields
        .iter()
        .map(|field| {
            field
                .source_type
                .as_ref()
                .map_or_else(|| field.name.clone(), |ty| format!("{}: {ty}", field.name))
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "{}struct {name} {{ {field_list} }}",
        visibility_prefix(visibility)
    )
}
