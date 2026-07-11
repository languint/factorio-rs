use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;

use crate::generate::ident::{make_ident, sanitize_doc};
use crate::generate::types::{KnownTypes, map_copy_field_type};
use crate::schema::{ApiType, Concept, RuntimeApi};

pub fn generatable_concept_names(
    api: &RuntimeApi,
    excluded: &BTreeSet<String>,
) -> BTreeSet<String> {
    api.concepts
        .iter()
        .filter(|c| {
            !excluded.contains(&c.name)
                && !c.type_name.is_homog_string_literal_union()
                && (concept_table_params(&c.type_name).is_some() || is_string_alias(c))
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
    let tokens = quote! { #( #items )* };
    format!("{header}{tokens}")
}

fn generate_concept(concept: &Concept, known: &KnownTypes<'_>) -> Option<TokenStream> {
    // Literal-union concepts are emitted by `generate_unions`.
    if concept.type_name.is_homog_string_literal_union() {
        return None;
    }

    let name = make_ident(&concept.name);
    let doc: Option<String> = if concept.description.is_empty() {
        None
    } else {
        Some(sanitize_doc(&concept.description))
    };

    if is_string_alias(concept) {
        return Some(match doc {
            Some(d) => quote! {
                #[doc = #d]
                pub type #name = &'static str;
            },
            None => quote! { pub type #name = &'static str; },
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

    let fields = params.iter().map(|(field_name, field_type, _optional)| {
        let ident = make_ident(field_name);
        let ty = map_copy_field_type(field_type, known);
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

fn concept_table_params(ty: &ApiType) -> Option<Vec<(String, ApiType, bool)>> {
    match ty.complex_type() {
        Some("table") => Some(ty.parameters()),
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
