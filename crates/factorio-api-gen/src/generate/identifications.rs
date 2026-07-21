//! Mixed Identification-style unions as Copy enums in `crate::concepts`.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::quote;

use crate::generate::concepts::concept_table_params;
use crate::generate::ident::{make_ident, sanitize_doc, to_pascal_case};
use crate::generate::types::{KnownTypes, is_integer_api_type_pub, map_numeric_type_tokens};
use crate::schema::{ApiType, Concept, RuntimeApi};

/// Concepts that stay as open `LuaAny` (too open or not useful as enums).
const SKIP: &[&str] = &["Any", "AnyBasic", "LocalisedString"];

/// Collect Identification-style concept names that should become enums.
pub fn identification_concept_names(
    api: &RuntimeApi,
    excluded: &BTreeSet<String>,
) -> BTreeSet<String> {
    api.concepts
        .iter()
        .filter(|c| is_identification_candidate(c, excluded))
        .map(|c| c.name.clone())
        .collect()
}

/// Build a lookup from sorted arm signature keys to identification concept name.
pub fn identification_signatures(
    api: &RuntimeApi,
    identification_names: &BTreeSet<String>,
) -> std::collections::HashMap<Vec<String>, String> {
    let mut map = std::collections::HashMap::new();
    for concept in &api.concepts {
        if !identification_names.contains(&concept.name) {
            continue;
        }
        let mut keys: Vec<String> = concept
            .type_name
            .non_nil_options()
            .iter()
            .filter_map(arm_signature_key)
            .collect();
        if keys.len() != concept.type_name.non_nil_options().len() {
            continue;
        }
        keys.sort();
        map.entry(keys).or_insert_with(|| concept.name.clone());
    }
    map
}

fn arm_signature_key(api_type: &ApiType) -> Option<String> {
    let arm = unwrap_type(api_type);
    if let Some(name) = arm.as_simple_name() {
        return Some(name.to_string());
    }
    match arm.complex_type() {
        Some("array") => {
            let value = arm.child_type("value")?;
            Some(format!("array<{}>", arm_signature_key(&value)?))
        }
        _ => None,
    }
}

fn is_identification_candidate(concept: &Concept, excluded: &BTreeSet<String>) -> bool {
    if excluded.contains(&concept.name) || SKIP.contains(&concept.name.as_str()) {
        return false;
    }
    if concept.type_name.complex_type() != Some("union") {
        return false;
    }
    if concept.type_name.is_homog_string_literal_union() {
        return false;
    }
    // Table-shaped unions (MapPosition, BoundingBox, ...) stay as structs.
    if concept_table_params(&concept.type_name).is_some() {
        return false;
    }
    // Only emit when every arm can be represented as a Copy payload.
    concept
        .type_name
        .non_nil_options()
        .iter()
        .all(|arm| arm_payload_kind(arm).is_some())
}

/// Generate Identification enums into the concepts module.
pub fn generate_identifications(api: &RuntimeApi, known: &KnownTypes<'_>) -> TokenStream {
    let by_name: BTreeMap<&str, &Concept> = api
        .concepts
        .iter()
        .map(|concept| (concept.name.as_str(), concept))
        .collect();
    let items = api
        .concepts
        .iter()
        .filter(|c| known.identifications.contains(&c.name))
        .filter_map(|concept| generate_identification(concept, known, &by_name));
    quote! { #( #items )* }
}

fn generate_identification(
    concept: &Concept,
    known: &KnownTypes<'_>,
    by_name: &BTreeMap<&str, &Concept>,
) -> Option<TokenStream> {
    let name = make_ident(&concept.name);
    let doc: Option<String> = if concept.description.is_empty() {
        None
    } else {
        Some(sanitize_doc(&concept.description))
    };

    let arms = concept.type_name.non_nil_options();
    let mut variants = Vec::new();
    let mut from_impls = Vec::new();
    let mut used_variant_names = BTreeSet::new();
    let mut used_from_tys = BTreeSet::new();
    let mut first_arm_name = None;
    let mut default_expr = None;
    let mut default_from_many = false;

    for (index, arm) in arms.iter().enumerate() {
        let (variant_name, payload_ty, from_ty) = arm_tokens(arm, known, &concept.name)?;
        let mut variant = variant_name;
        if used_variant_names.contains(&variant) {
            let mut suffix = 2u32;
            while used_variant_names.contains(&format!("{variant}{suffix}")) {
                suffix = suffix.saturating_add(1);
            }
            variant = format!("{variant}{suffix}");
        }
        used_variant_names.insert(variant.clone());
        let variant_ident = make_ident(&variant);
        let is_many = matches!(arm_payload_kind(arm), Some(ArmKind::Array(_)));
        if first_arm_name.is_none() || (default_from_many && !is_many) {
            first_arm_name = Some(variant_ident.clone());
            default_expr = Some(default_expr_for_arm(arm, known)?);
            default_from_many = is_many;
        }
        let _ = index;
        variants.push(quote! { #variant_ident(#payload_ty) });
        let from_key = from_ty.to_string();
        if used_from_tys.insert(from_key) {
            from_impls.push(quote! {
                impl From<#from_ty> for #name {
                    fn from(value: #from_ty) -> Self {
                        Self::#variant_ident(value)
                    }
                }
            });
        }

        // `ForceSet::One(ForceID)` - also accept ForceID payload types directly.
        if variant == "One"
            && let Some(inner) = one_inner_concept_name(arm)
            && let Some(inner_concept) = by_name.get(inner.as_str())
        {
            for inner_arm in inner_concept.type_name.non_nil_options() {
                let Some((_inner_variant, inner_payload, _)) =
                    arm_tokens(&inner_arm, known, &inner)
                else {
                    continue;
                };
                let from_key = inner_payload.to_string();
                if !used_from_tys.insert(from_key) {
                    continue;
                }
                let inner_ident = make_ident(&inner);
                from_impls.push(quote! {
                    impl From<#inner_payload> for #name {
                        fn from(value: #inner_payload) -> Self {
                            Self::#variant_ident(#inner_ident::from(value))
                        }
                    }
                });
            }
        }
    }

    if variants.is_empty() {
        return None;
    }

    let first_arm_name = first_arm_name?;
    let default_expr = default_expr?;

    Some(match doc {
        Some(d) => quote! {
            #[doc = #d]
            #[derive(Debug, Clone, Copy, PartialEq)]
            pub enum #name {
                #( #variants , )*
            }

            impl Default for #name {
                fn default() -> Self {
                    Self::#first_arm_name(#default_expr)
                }
            }

            #( #from_impls )*

            impl From<#name> for crate::LuaAny {
                fn from(_: #name) -> Self {
                    crate::LuaAny
                }
            }
        },
        None => quote! {
            #[derive(Debug, Clone, Copy, PartialEq)]
            pub enum #name {
                #( #variants , )*
            }

            impl Default for #name {
                fn default() -> Self {
                    Self::#first_arm_name(#default_expr)
                }
            }

            #( #from_impls )*

            impl From<#name> for crate::LuaAny {
                fn from(_: #name) -> Self {
                    crate::LuaAny
                }
            }
        },
    })
}

enum ArmKind {
    String,
    Numeric(String),
    Class(String),
    Concept(String),
    Array(Box<ArmKind>),
}

fn arm_payload_kind(arm: &ApiType) -> Option<ArmKind> {
    let arm = unwrap_type(arm);
    if let Some(name) = arm.as_simple_name() {
        return match name {
            "string" | "LocalisedString" => Some(ArmKind::String),
            "boolean" => None,
            n if is_integer_api_type_pub(n) || matches!(n, "float" | "double" | "number") => {
                Some(ArmKind::Numeric(n.to_string()))
            }
            other if other.starts_with("defines.") => Some(ArmKind::String),
            other if other.starts_with("Lua") => Some(ArmKind::Class(other.to_string())),
            other => Some(ArmKind::Concept(other.to_string())),
        };
    }
    match arm.complex_type() {
        Some("array") => {
            let value = arm.child_type("value")?;
            Some(ArmKind::Array(Box::new(arm_payload_kind(&value)?)))
        }
        Some("literal") => None,
        _ => None,
    }
}

fn unwrap_type(api_type: &ApiType) -> ApiType {
    if api_type.complex_type() == Some("type")
        && let Some(inner) = api_type.child_type("value")
    {
        return unwrap_type(&inner);
    }
    api_type.clone()
}

fn one_inner_concept_name(arm: &ApiType) -> Option<String> {
    match arm_payload_kind(arm)? {
        ArmKind::Concept(name) => Some(name),
        _ => None,
    }
}

fn arm_tokens(
    arm: &ApiType,
    known: &KnownTypes<'_>,
    parent: &str,
) -> Option<(String, TokenStream, TokenStream)> {
    let kind = arm_payload_kind(arm)?;
    Some(arm_kind_tokens(&kind, known, parent))
}

fn arm_kind_tokens(
    kind: &ArmKind,
    known: &KnownTypes<'_>,
    parent: &str,
) -> (String, TokenStream, TokenStream) {
    match kind {
        ArmKind::String => (
            "Name".to_string(),
            quote!(&'static str),
            quote!(&'static str),
        ),
        ArmKind::Numeric(n) => {
            let ty = map_numeric_type_tokens(n);
            let variant = match n.as_str() {
                "float" | "double" | "number" => "Number",
                _ => "Index",
            };
            (variant.to_string(), ty.clone(), ty)
        }
        ArmKind::Class(name) => {
            let ident = make_ident(name);
            let variant = class_variant_name(name);
            (
                variant,
                quote!(crate::classes::#ident),
                quote!(crate::classes::#ident),
            )
        }
        ArmKind::Concept(name) => {
            let ident = make_ident(name);
            let variant = concept_variant_name(name, parent);
            let _ = known;
            (
                variant,
                quote!(crate::concepts::#ident),
                quote!(crate::concepts::#ident),
            )
        }
        ArmKind::Array(inner) => {
            let (_, inner_ty, _) = arm_kind_tokens(inner, known, parent);
            let variant = "Many".to_string();
            (
                variant,
                quote!(&'static [#inner_ty]),
                quote!(&'static [#inner_ty]),
            )
        }
    }
}

fn class_variant_name(class_name: &str) -> String {
    let trimmed = class_name.strip_prefix("Lua").unwrap_or(class_name);
    to_pascal_case(trimmed)
}

fn concept_variant_name(concept_name: &str, parent: &str) -> String {
    if concept_name == parent {
        return "One".to_string();
    }
    if concept_name.ends_with("Table") {
        return "Table".to_string();
    }
    if concept_name == "MapPosition" {
        return "Position".to_string();
    }
    if concept_name.ends_with("ID")
        || concept_name.ends_with("Identification")
        || concept_name.ends_with("Set")
        || concept_name.ends_with("Pair")
        || concept_name.ends_with("Definition")
        || concept_name == "Fluid"
        || concept_name.ends_with("Product")
        || concept_name.ends_with("Goal")
    {
        // Nested identification / table payloads keep a readable name; ForceSet's
        // single-ForceID arm becomes `One`.
        if parent.ends_with("Set")
            && (concept_name.ends_with("ID") || concept_name.ends_with("Identification"))
        {
            return "One".to_string();
        }
        return to_pascal_case(concept_name);
    }
    to_pascal_case(concept_name)
}

fn default_expr_for_arm(arm: &ApiType, known: &KnownTypes<'_>) -> Option<TokenStream> {
    let kind = arm_payload_kind(arm)?;
    let _ = known;
    Some(match kind {
        ArmKind::String => quote!(""),
        ArmKind::Numeric(n) => {
            if matches!(n.as_str(), "float" | "double" | "number") {
                quote!(0.0)
            } else {
                quote!(0)
            }
        }
        ArmKind::Class(_) | ArmKind::Concept(_) => quote!(Default::default()),
        ArmKind::Array(_) => quote!(&[]),
    })
}
