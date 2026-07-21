use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;

use crate::generate::ident::{make_ident, sanitize_doc, sanitize_ident, to_pascal_case};
use crate::generate::types::{
    KnownTypes, map_copy_field_type_for_concept, map_numeric_type_tokens,
};
use crate::schema::{ApiType, Concept, RuntimeApi};

/// Dictionary concepts emitted as `{ flags: &'static [&str] }` (dict-of-true in Lua).
const FLAG_SET_CONCEPTS: &[&str] = &[
    "MouseButtonFlags",
    "SelectionModeFlags",
    "EntityPrototypeFlags",
    "ItemPrototypeFlags",
    "TriggerTargetMask",
];

/// Named concepts that are always registered even when not table-shaped.
const EXTRA_CONCEPTS: &[&str] = &[
    "Tags",
    "MapGenSize",
    "RenderLayer",
    "PropertyExpressionNames",
    "EventFilter",
    "MouseButtonFlags",
    "SelectionModeFlags",
    "EntityPrototypeFlags",
    "ItemPrototypeFlags",
    "TriggerTargetMask",
];

pub fn generatable_concept_names(
    api: &RuntimeApi,
    excluded: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = api
        .concepts
        .iter()
        .filter(|c| {
            !excluded.contains(&c.name)
                && !c.type_name.is_homog_string_literal_union()
                && (concept_table_params(&c.type_name).is_some()
                    || is_string_alias(c)
                    || is_numeric_alias(c)
                    || is_special_concept(&c.name)
                    || c.type_name.is_flag_set_dictionary())
        })
        .map(|c| c.name.clone())
        .collect();
    for extra in EXTRA_CONCEPTS {
        if !excluded.contains(*extra) {
            names.insert((*extra).to_string());
        }
    }
    names
}

/// Factorio flag-set concept names (dict of active keys -> `true`).
#[must_use]
pub fn flag_set_concept_names(api: &RuntimeApi) -> BTreeSet<String> {
    api.concepts
        .iter()
        .filter(|c| {
            FLAG_SET_CONCEPTS.contains(&c.name.as_str()) || c.type_name.is_flag_set_dictionary()
        })
        .map(|c| c.name.clone())
        .collect()
}

/// Returns the set of concept names used as event filters.
pub fn event_filter_concept_names(api: &RuntimeApi) -> BTreeSet<String> {
    api.events
        .iter()
        .filter_map(|event| event.filter.clone())
        .collect()
}

pub fn generate_concepts(
    api: &RuntimeApi,
    known: &KnownTypes<'_>,
    excluded: &BTreeSet<String>,
) -> String {
    let header = "#![allow(nonstandard_style)]\n";
    let items = api.concepts.iter().filter_map(|concept| {
        if excluded.contains(&concept.name) {
            return None;
        }
        generate_concept(concept, known)
    });
    let flag_helper = generate_flag_set_helper(known);
    let tokens = quote! {
        #( #items )*
        #flag_helper
    };
    format!("{header}{tokens}")
}

fn generate_flag_set_helper(known: &KnownTypes<'_>) -> TokenStream {
    if known.flag_sets.is_empty() {
        return quote! {
            /// Whether `name` is a Factorio flag-set concept (`MouseButtonFlags`, ...).
            #[must_use]
            pub fn is_flag_set_type(name: &str) -> bool {
                false
            }
        };
    }
    let arms = known.flag_sets.iter().map(|name| {
        let lit = name.as_str();
        quote!(#lit)
    });
    quote! {
        /// Whether `name` is a Factorio flag-set concept (`MouseButtonFlags`, ...).
        #[must_use]
        pub fn is_flag_set_type(name: &str) -> bool {
            matches!(name, #( #arms )|*)
        }
    }
}

fn generate_concept(concept: &Concept, known: &KnownTypes<'_>) -> Option<TokenStream> {
    // Literal-union concepts are emitted by `generate_unions`.
    if concept.type_name.is_homog_string_literal_union() {
        return None;
    }

    // Identification enums are emitted by `generate_identifications`.
    if known.identifications.contains(&concept.name) {
        return None;
    }

    let name = make_ident(&concept.name);
    let doc: Option<String> = if concept.description.is_empty() {
        None
    } else {
        Some(sanitize_doc(&concept.description))
    };

    if concept.name == "EventFilter" {
        // Typed as `Vec<EventFilterEntry>` via `map_simple_type`; no struct.
        return None;
    }

    if concept.name == "Tags" {
        return Some(generate_tags_concept(doc.as_deref()));
    }

    if concept.name == "PropertyExpressionNames" {
        return Some(generate_string_map_concept(
            &concept.name,
            doc.as_deref(),
            "pairs",
        ));
    }

    if concept.name == "MapGenSize" {
        return Some(generate_map_gen_size(doc.as_deref()));
    }

    if concept.name == "RenderLayer" {
        return Some(generate_named_string_enum(
            &concept.name,
            doc.as_deref(),
            &concept.type_name,
        ));
    }

    if known.flag_sets.contains(&concept.name) || concept.type_name.is_flag_set_dictionary() {
        return Some(generate_flag_set_concept(&concept.name, doc.as_deref()));
    }

    if is_string_alias(concept) {
        return Some(match doc {
            Some(d) => quote! {
                #[doc = #d]
                pub type #name = &'static str;
            },
            None => quote! { pub type #name = &'static str; },
        });
    }

    if let Some(underlying) = numeric_alias_underlying(concept) {
        let rust_ty = map_numeric_type_tokens(underlying);
        return Some(match doc {
            Some(d) => quote! {
                #[doc = #d]
                pub type #name = #rust_ty;
            },
            None => quote! { pub type #name = #rust_ty; },
        });
    }

    let params = concept_table_params(&concept.type_name)?;
    if params.is_empty() {
        return Some(match doc {
            Some(d) => quote! {
                #[doc = #d]
                #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
                pub struct #name;
            },
            None => quote! {
                #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
                pub struct #name;
            },
        });
    }

    let fields = params.iter().map(|(field_name, field_type, optional)| {
        let ident = make_ident(field_name);
        let base = map_copy_field_type_for_concept(field_type, known, &concept.name);
        let ty = if *optional {
            quote!(Option<#base>)
        } else {
            base
        };
        quote! { pub #ident: #ty, }
    });

    Some(match doc {
        Some(d) => quote! {
            #[doc = #d]
            #[derive(Debug, Clone, Copy, PartialEq, Default)]
            pub struct #name {
                #( #fields )*
            }
        },
        None => quote! {
            #[derive(Debug, Clone, Copy, PartialEq, Default)]
            pub struct #name {
                #( #fields )*
            }
        },
    })
}

fn generate_flag_set_concept(concept_name: &str, doc: Option<&str>) -> TokenStream {
    let name = make_ident(concept_name);
    let doc_attr = doc.map(|d| quote!(#[doc = #d]));
    quote! {
        #doc_attr
        /// Factorio flag set: active keys present as `true` in a Lua table.
        ///
        /// Construct with `flags: &["left", "right"]`. Lowers to
        /// `{ ["left"] = true, ["right"] = true }`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct #name {
            pub flags: &'static [&'static str],
        }
    }
}

fn generate_tags_concept(doc: Option<&str>) -> TokenStream {
    let doc_attr = doc.map(|d| quote!(#[doc = #d]));
    quote! {
        #doc_attr
        /// One string entry in a [`Tags`] table.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct TagPair {
            pub key: &'static str,
            pub value: &'static str,
        }

        /// Factorio `Tags` dictionary (`string` -> `AnyBasic`).
        ///
        /// String values can be built with [`Tags::pairs`]. Non-string tag values
        /// still use open [`crate::LuaAny`] helpers on the returned table.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct Tags {
            pub pairs: &'static [TagPair],
        }

        impl Tags {
            #[must_use]
            pub const fn empty() -> Self {
                Self { pairs: &[] }
            }

            #[must_use]
            pub const fn pairs(pairs: &'static [TagPair]) -> Self {
                Self { pairs }
            }
        }
    }
}

fn generate_string_map_concept(concept_name: &str, doc: Option<&str>, field: &str) -> TokenStream {
    let name = make_ident(concept_name);
    let field_ident = make_ident(field);
    let pair_name = make_ident(&format!("{concept_name}Pair"));
    let doc_attr = doc.map(|d| quote!(#[doc = #d]));
    quote! {
        #doc_attr
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct #pair_name {
            pub key: &'static str,
            pub value: &'static str,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct #name {
            pub #field_ident: &'static [#pair_name],
        }
    }
}

fn generate_map_gen_size(doc: Option<&str>) -> TokenStream {
    let doc_attr = doc.map(|d| quote!(#[doc = #d]));
    quote! {
        #doc_attr
        /// Map generation size: a number or a named preset string.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum MapGenSize {
            Number(f32),
            Named(&'static str),
        }

        impl Default for MapGenSize {
            fn default() -> Self {
                Self::Named("none")
            }
        }

        impl From<f32> for MapGenSize {
            fn from(value: f32) -> Self {
                Self::Number(value)
            }
        }

        impl From<&'static str> for MapGenSize {
            fn from(value: &'static str) -> Self {
                Self::Named(value)
            }
        }

        impl From<MapGenSize> for crate::LuaAny {
            fn from(_: MapGenSize) -> Self {
                crate::LuaAny
            }
        }
    }
}

fn generate_named_string_enum(concept_name: &str, doc: Option<&str>, ty: &ApiType) -> TokenStream {
    let name = make_ident(concept_name);
    let doc_attr = doc.map(|d| quote!(#[doc = #d]));
    let mut consts = Vec::new();
    for option in ty.non_nil_options() {
        if option.literal_kind() != Some("string") {
            continue;
        }
        let Some(lit) = option.0.get("value").and_then(|v| v.as_str()) else {
            continue;
        };
        let variant = literal_to_const_name(lit);
        let ident = make_ident(&variant);
        consts.push(quote! {
            pub const #ident: Self = Self::Named(#lit);
        });
    }
    quote! {
        #doc_attr
        /// Named render layer string (or a numeric string).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum #name {
            Named(&'static str),
        }

        impl #name {
            #( #consts )*
        }

        impl Default for #name {
            fn default() -> Self {
                Self::Named("object")
            }
        }

        impl From<&'static str> for #name {
            fn from(value: &'static str) -> Self {
                Self::Named(value)
            }
        }

        impl From<#name> for crate::LuaAny {
            fn from(_: #name) -> Self {
                crate::LuaAny
            }
        }
    }
}

fn literal_to_const_name(literal: &str) -> String {
    let normalized = literal.replace(['-', ' ', '/'], "_");
    let pascal = to_pascal_case(&normalized);
    let sanitized = sanitize_ident(&pascal);
    if sanitized.is_empty() || sanitized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("Value_{sanitized}")
    } else {
        sanitized
    }
}

fn is_special_concept(name: &str) -> bool {
    EXTRA_CONCEPTS.contains(&name)
}

pub(crate) fn concept_table_params(ty: &ApiType) -> Option<Vec<(String, ApiType, bool)>> {
    match ty.complex_type() {
        Some("table") => Some(ty.parameters()),
        Some("LuaStruct") => Some(ty.attributes()),
        Some("union") => ty
            .options()
            .into_iter()
            .find(|opt| opt.complex_type() == Some("table"))
            .map(|t| t.parameters()),
        _ => None,
    }
}

fn is_string_alias(concept: &Concept) -> bool {
    matches!(concept.type_name.as_simple_name(), Some("string"))
}

fn is_numeric_alias(concept: &Concept) -> bool {
    numeric_alias_underlying(concept).is_some()
}

fn numeric_alias_underlying(concept: &Concept) -> Option<&'static str> {
    let name = concept.type_name.as_simple_name()?;
    match name {
        "float" => Some("float"),
        "double" => Some("double"),
        "number" => Some("number"),
        "uint8" => Some("uint8"),
        "uint16" => Some("uint16"),
        "uint32" => Some("uint32"),
        "uint64" => Some("uint64"),
        "uint" => Some("uint"),
        "int8" => Some("int8"),
        "int16" => Some("int16"),
        "int32" => Some("int32"),
        "int64" => Some("int64"),
        "int" => Some("int"),
        _ => None,
    }
}
