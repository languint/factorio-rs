use syn::{Receiver, Type};

use crate::error::{FrontendError, FrontendResult};

use super::util::location;

pub fn lower_type(ty: &Type) -> FrontendResult<factorio_ir::r#type::Type> {
    match ty {
        Type::Path(path) => lower_path_type(path),
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(factorio_ir::r#type::Type::Void),
        Type::Reference(reference) if is_self_type(&reference.elem) => {
            Ok(factorio_ir::r#type::Type::Void)
        }
        Type::Reference(reference) => {
            // &str and &'static str map to Str
            if let Type::Path(inner) = reference.elem.as_ref()
                && inner.path.is_ident("str")
            {
                return Ok(factorio_ir::r#type::Type::Str);
            }
            Err(FrontendError::UnsupportedType {
                ty: "unsupported reference type".to_string(),
                location: location(ty),
            })
        }
        _ => Err(FrontendError::UnsupportedType {
            ty: "unsupported type".to_string(),
            location: location(ty),
        }),
    }
}

fn lower_path_type(path: &syn::TypePath) -> FrontendResult<factorio_ir::r#type::Type> {
    let segment = path
        .path
        .segments
        .last()
        .ok_or_else(|| FrontendError::UnsupportedType {
            ty: "empty path".to_string(),
            location: location(path),
        })?;

    let ty = match segment.ident.to_string().as_str() {
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => factorio_ir::r#type::Type::Int,
        "f32" | "f64" => factorio_ir::r#type::Type::Float,
        "str" | "String" => factorio_ir::r#type::Type::Str,
        _ => factorio_ir::r#type::Type::Void,
    };

    Ok(ty)
}

fn is_self_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident("Self"))
}

pub fn lower_binding(
    pattern: &syn::Pat,
) -> FrontendResult<(String, Option<(factorio_ir::r#type::Type, String)>)> {
    match pattern {
        syn::Pat::Type(pat_type) => {
            let name = lower_binding_pattern(&pat_type.pat)?;
            let ty = lower_type(&pat_type.ty)?;
            let source_type = type_source_string(&pat_type.ty);
            Ok((name, Some((ty, source_type))))
        }
        pattern => {
            let name = lower_binding_pattern(pattern)?;
            Ok((name, None))
        }
    }
}

pub fn lower_binding_pattern(pattern: &syn::Pat) -> FrontendResult<String> {
    match pattern {
        syn::Pat::Ident(ident) => Ok(ident.ident.to_string()),
        syn::Pat::Type(pat_type) => lower_binding_pattern(&pat_type.pat),
        syn::Pat::Wild(_) => Ok("_".to_string()),
        _ => Err(FrontendError::ExpectedIdentifierPattern {
            location: location(pattern),
        }),
    }
}

pub const fn infer_type_from_expression(
    expression: &factorio_ir::expression::Expression,
) -> Option<factorio_ir::r#type::Type> {
    match expression {
        factorio_ir::expression::Expression::Literal(literal) => match literal {
            factorio_ir::literal::Literal::Int(_) => Some(factorio_ir::r#type::Type::Int),
            factorio_ir::literal::Literal::Float(_) => Some(factorio_ir::r#type::Type::Float),
            factorio_ir::literal::Literal::String(_) => Some(factorio_ir::r#type::Type::Str),
            factorio_ir::literal::Literal::Bool(_) | factorio_ir::literal::Literal::Nil => None,
        },
        _ => None,
    }
}

/// Last-segment type name for Debug format selection (`Option` / references peeled).
#[must_use]
pub fn rust_type_key(ty: &Type) -> Option<String> {
    match ty {
        Type::Reference(reference) => rust_type_key(&reference.elem),
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            let name = segment.ident.to_string();
            if matches!(name.as_str(), "Option" | "Box")
                && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
            {
                for arg in &args.args {
                    if let syn::GenericArgument::Type(inner) = arg {
                        return rust_type_key(inner);
                    }
                }
            }
            Some(name)
        }
        _ => None,
    }
}

pub fn type_source_string(ty: &Type) -> String {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        Type::Reference(reference) => {
            let mut source = String::from("&");
            if reference.mutability.is_some() {
                source.push_str("mut ");
            }
            source.push_str(&type_source_string(&reference.elem));
            source
        }
        Type::Tuple(tuple) if tuple.elems.is_empty() => "()".to_string(),
        Type::Tuple(tuple) => {
            let elements = tuple
                .elems
                .iter()
                .map(type_source_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({elements})")
        }
        _ => "unsupported".to_string(),
    }
}

pub fn receiver_source_string(receiver: &Receiver) -> String {
    let mut source = String::from("&");
    if receiver.mutability.is_some() {
        source.push_str("mut ");
    }
    source.push_str("self");
    source
}

pub fn return_type_string(signature: &syn::Signature) -> Option<String> {
    match &signature.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => Some(type_source_string(ty)),
    }
}

pub fn inferred_source_type(ty: &factorio_ir::r#type::Type) -> Option<String> {
    match ty {
        factorio_ir::r#type::Type::Int => Some("integer".to_string()),
        factorio_ir::r#type::Type::Float => Some("float".to_string()),
        factorio_ir::r#type::Type::Str => Some("str".to_string()),
        factorio_ir::r#type::Type::Void => None,
    }
}
