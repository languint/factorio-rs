use std::collections::{HashMap, HashSet};

use syn::{GenericArgument, GenericParam, PathArguments, Type, TypeParamBound, TypePath};

use crate::error::{FrontendError, FrontendResult};

use super::util::location;

/// A `type Name = ...` / `type Name<T> = ...` binding for transparent resolution.
#[derive(Clone)]
pub struct TypeAlias {
    pub params: Vec<String>,
    pub ty: Type,
}

/// Resolve type aliases (and nested aliases) before IR/binding helpers run.
#[must_use]
pub fn resolve_type(
    ty: &Type,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
) -> Type {
    let mut stack = HashSet::new();
    resolve_type_rec(ty, aliases, assoc, &mut stack)
}

fn resolve_type_rec(
    ty: &Type,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
    stack: &mut HashSet<String>,
) -> Type {
    match ty {
        Type::Reference(reference) => {
            let mut resolved = reference.clone();
            resolved.elem = Box::new(resolve_type_rec(&reference.elem, aliases, assoc, stack));
            Type::Reference(resolved)
        }
        Type::Tuple(tuple) => {
            let mut resolved = tuple.clone();
            for elem in &mut resolved.elems {
                *elem = resolve_type_rec(elem, aliases, assoc, stack);
            }
            Type::Tuple(resolved)
        }
        Type::Path(path) => resolve_path_type(path, aliases, assoc, stack),
        other => other.clone(),
    }
}

fn resolve_path_type(
    path: &TypePath,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
    stack: &mut HashSet<String>,
) -> Type {
    // `Self::AssocName` -> concrete type from the active trait impl.
    if path.qself.is_none()
        && path.path.segments.len() == 2
        && path.path.segments[0].ident == "Self"
        && matches!(path.path.segments[0].arguments, PathArguments::None)
        && matches!(path.path.segments[1].arguments, PathArguments::None)
    {
        let assoc_name = path.path.segments[1].ident.to_string();
        if let Some(replacement) = assoc.get(&assoc_name) {
            return resolve_type_rec(replacement, aliases, assoc, stack);
        }
    }

    let Some(segment) = path.path.segments.last() else {
        return Type::Path(path.clone());
    };
    let name = segment.ident.to_string();

    // Resolve generic arguments even when the path itself is not an alias.
    let resolved_args = resolve_path_arguments(&segment.arguments, aliases, assoc, stack);

    let Some(alias) = aliases.get(&name) else {
        let mut path = path.clone();
        if let Some(last) = path.path.segments.last_mut() {
            last.arguments = resolved_args;
        }
        return Type::Path(path);
    };

    if !stack.insert(name.clone()) {
        // Cycle, leave the written type alone.
        return Type::Path(path.clone());
    }

    let subst = match &resolved_args {
        PathArguments::AngleBracketed(args) => {
            let type_args: Vec<Type> = args
                .args
                .iter()
                .filter_map(|arg| match arg {
                    GenericArgument::Type(inner) => Some(inner.clone()),
                    _ => None,
                })
                .collect();
            alias
                .params
                .iter()
                .cloned()
                .zip(type_args)
                .collect::<HashMap<_, _>>()
        }
        _ => HashMap::new(),
    };

    let substituted = substitute_type(&alias.ty, &subst);
    let resolved = resolve_type_rec(&substituted, aliases, assoc, stack);
    stack.remove(&name);
    resolved
}

fn resolve_path_arguments(
    arguments: &PathArguments,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
    stack: &mut HashSet<String>,
) -> PathArguments {
    match arguments {
        PathArguments::AngleBracketed(args) => {
            let mut args = args.clone();
            for arg in &mut args.args {
                if let GenericArgument::Type(inner) = arg {
                    *inner = resolve_type_rec(inner, aliases, assoc, stack);
                }
            }
            PathArguments::AngleBracketed(args)
        }
        other => other.clone(),
    }
}

fn substitute_type(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Path(path)
            if path.qself.is_none()
                && path.path.segments.len() == 1
                && matches!(
                    path.path.segments[0].arguments,
                    PathArguments::None | PathArguments::AngleBracketed(_)
                ) =>
        {
            let segment = &path.path.segments[0];
            let name = segment.ident.to_string();
            if matches!(segment.arguments, PathArguments::None)
                && let Some(replacement) = subst.get(&name)
            {
                return replacement.clone();
            }
            let mut path = path.clone();
            if let Some(last) = path.path.segments.last_mut()
                && let PathArguments::AngleBracketed(args) = &mut last.arguments
            {
                for arg in &mut args.args {
                    if let GenericArgument::Type(inner) = arg {
                        *inner = substitute_type(inner, subst);
                    }
                }
            }
            Type::Path(path)
        }
        Type::Reference(reference) => {
            let mut resolved = reference.clone();
            resolved.elem = Box::new(substitute_type(&reference.elem, subst));
            Type::Reference(resolved)
        }
        Type::Tuple(tuple) => {
            let mut resolved = tuple.clone();
            for elem in &mut resolved.elems {
                *elem = substitute_type(elem, subst);
            }
            Type::Tuple(resolved)
        }
        other => other.clone(),
    }
}

/// Register a top-level / nested `type` item. Emits no IR.
pub fn register_type_alias(
    item: &syn::ItemType,
    aliases: &mut HashMap<String, TypeAlias>,
) -> FrontendResult<()> {
    if item.generics.where_clause.is_some() {
        return Err(FrontendError::UnsupportedItem {
            item: "type alias with where-clause".to_string(),
            location: location(item),
        });
    }

    let mut params = Vec::new();
    for param in &item.generics.params {
        match param {
            GenericParam::Type(type_param) => {
                if !type_param.bounds.is_empty() {
                    return Err(FrontendError::UnsupportedItem {
                        item: "type alias parameter bounds".to_string(),
                        location: location(type_param),
                    });
                }
                params.push(type_param.ident.to_string());
            }
            GenericParam::Lifetime(lifetime) => {
                return Err(FrontendError::UnsupportedItem {
                    item: "type alias lifetime parameter".to_string(),
                    location: location(lifetime),
                });
            }
            GenericParam::Const(const_param) => {
                return Err(FrontendError::UnsupportedItem {
                    item: "type alias const parameter".to_string(),
                    location: location(const_param),
                });
            }
        }
    }

    aliases.insert(
        item.ident.to_string(),
        TypeAlias {
            params,
            ty: item.ty.as_ref().clone(),
        },
    );
    Ok(())
}

/// Collect `type` aliases from a module item list (and nested inline mods).
pub fn collect_type_aliases(
    items: &[syn::Item],
    aliases: &mut HashMap<String, TypeAlias>,
) -> FrontendResult<()> {
    for item in items {
        match item {
            syn::Item::Type(item_type) => register_type_alias(item_type, aliases)?,
            syn::Item::Mod(item_mod) => {
                if let Some((_, nested)) = &item_mod.content {
                    collect_type_aliases(nested, aliases)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn lower_type(
    ty: &Type,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
) -> FrontendResult<factorio_ir::r#type::Type> {
    lower_type_resolved(&resolve_type(ty, aliases, assoc))
}

fn lower_type_resolved(ty: &Type) -> FrontendResult<factorio_ir::r#type::Type> {
    match ty {
        Type::Path(path) => lower_path_type(path),
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(factorio_ir::r#type::Type::Void),
        Type::TraitObject(_) => Ok(factorio_ir::r#type::Type::Void),
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
            // `&dyn Trait` / `&mut dyn Trait`
            if matches!(reference.elem.as_ref(), Type::TraitObject(_)) {
                return Ok(factorio_ir::r#type::Type::Void);
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
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
) -> FrontendResult<(String, Option<(factorio_ir::r#type::Type, String)>)> {
    match pattern {
        syn::Pat::Type(pat_type) => {
            let name = lower_binding_pattern(&pat_type.pat)?;
            let ty = lower_type(&pat_type.ty, aliases, assoc)?;
            let source_type = type_source_string(&pat_type.ty, aliases, assoc);
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
pub fn rust_type_key(
    ty: &Type,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
) -> Option<String> {
    rust_type_key_resolved(&resolve_type(ty, aliases, assoc))
}

fn rust_type_key_resolved(ty: &Type) -> Option<String> {
    match ty {
        Type::Reference(reference) => rust_type_key_resolved(&reference.elem),
        Type::TraitObject(obj) => {
            // Prefer the trait name for dyn bindings.
            obj.bounds.iter().find_map(|bound| {
                if let TypeParamBound::Trait(trait_bound) = bound {
                    trait_bound
                        .path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                } else {
                    None
                }
            })
        }
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            let name = segment.ident.to_string();
            if matches!(name.as_str(), "Option" | "Box")
                && let PathArguments::AngleBracketed(args) = &segment.arguments
            {
                for arg in &args.args {
                    if let GenericArgument::Type(inner) = arg {
                        return rust_type_key_resolved(inner);
                    }
                }
            }
            Some(name)
        }
        _ => None,
    }
}

/// `true` when `ty` is `Option<_>` (aliases resolved, references peeled).
#[must_use]
pub fn is_option_type(
    ty: &Type,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
) -> bool {
    is_option_type_resolved(&resolve_type(ty, aliases, assoc))
}

fn is_option_type_resolved(ty: &Type) -> bool {
    match ty {
        Type::Reference(reference) => is_option_type_resolved(&reference.elem),
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Option"),
        _ => false,
    }
}

#[must_use]
pub fn type_source_string(
    ty: &Type,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
) -> String {
    type_source_string_resolved(&resolve_type(ty, aliases, assoc))
}

fn type_source_string_resolved(ty: &Type) -> String {
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
            source.push_str(&type_source_string_resolved(&reference.elem));
            source
        }
        Type::Tuple(tuple) if tuple.elems.is_empty() => "()".to_string(),
        Type::Tuple(tuple) => {
            let elements = tuple
                .elems
                .iter()
                .map(type_source_string_resolved)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({elements})")
        }
        Type::TraitObject(obj) => {
            let bounds = obj
                .bounds
                .iter()
                .filter_map(|bound| match bound {
                    TypeParamBound::Trait(trait_bound) => Some(
                        trait_bound
                            .path
                            .segments
                            .iter()
                            .map(|segment| segment.ident.to_string())
                            .collect::<Vec<_>>()
                            .join("::"),
                    ),
                    TypeParamBound::Lifetime(_) => Some("Lifetime".to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" + ");
            format!("dyn {bounds}")
        }
        Type::Paren(paren) => type_source_string_resolved(&paren.elem),
        Type::Group(group) => type_source_string_resolved(&group.elem),
        _ => "unsupported".to_string(),
    }
}

pub fn receiver_source_string(receiver: &syn::Receiver) -> String {
    let mut source = String::from("&");
    if receiver.mutability.is_some() {
        source.push_str("mut ");
    }
    source.push_str("self");
    source
}

pub fn return_type_string(
    signature: &syn::Signature,
    aliases: &HashMap<String, TypeAlias>,
    assoc: &HashMap<String, Type>,
) -> Option<String> {
    match &signature.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => Some(type_source_string(ty, aliases, assoc)),
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
