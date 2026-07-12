//! Compile-time helpers for `{:?}` Debug lowering (JSON vs `tostring`).

use quote::quote;

use crate::generate::concepts::concept_table_params;
use crate::generate::events::event_rust_name;
use crate::generate::types::KnownTypes;
use crate::schema::{ApiType, RuntimeApi};

/// Emit `is_userdata_class` + `struct_field_type` for Debug format selection.
pub fn generate_debug_types(api: &RuntimeApi, known: &KnownTypes<'_>) -> String {
    let class_arms = known.classes.iter().map(|name| {
        let lit = name.as_str();
        quote!(#lit)
    });

    let identification_match = if known.identifications.is_empty() {
        quote! { false }
    } else {
        let ident_arms = known.identifications.iter().map(|name| {
            let lit = name.as_str();
            quote!(#lit)
        });
        quote! { matches!(name, #( #ident_arms )|*) }
    };

    let mut field_arms = Vec::new();
    for event in &api.events {
        let struct_name = format!("{}Event", event_rust_name(&event.name));
        for parameter in &event.data {
            let Some(ty_key) = debug_type_key(&parameter.type_name) else {
                continue;
            };
            let struct_lit = struct_name.as_str();
            let field_lit = parameter.name.as_str();
            let ty_lit = ty_key.as_str();
            field_arms.push(quote! {
                (#struct_lit, #field_lit) => Some(#ty_lit),
            });
        }
    }

    // Also index concept table fields (Color, MapPosition, ...).
    for concept in &api.concepts {
        if !known.concepts.contains(&concept.name) {
            continue;
        }
        let Some(params) = concept_table_params(&concept.type_name) else {
            continue;
        };
        for (name, ty, _) in params {
            let Some(ty_key) = debug_type_key(&ty) else {
                continue;
            };
            let struct_lit = concept.name.as_str();
            let field_lit = name.as_str();
            let ty_lit = ty_key.as_str();
            field_arms.push(quote! {
                (#struct_lit, #field_lit) => Some(#ty_lit),
            });
        }
    }

    quote! {
        /// Factorio runtime class names that are Lua userdata (not plain tables).
        #[must_use]
        pub fn is_userdata_class(name: &str) -> bool {
            matches!(name, #( #class_arms )|*)
        }

        /// Concept names generated as Identification-style enums (`ForceID`, ...).
        #[must_use]
        pub fn is_identification_type(name: &str) -> bool {
            #identification_match
        }

        /// Field type key for generated event/concept structs (last path segment form).
        #[must_use]
        pub fn struct_field_type(struct_name: &str, field: &str) -> Option<&'static str> {
            match (struct_name, field) {
                #( #field_arms )*
                _ => None,
            }
        }
    }
    .to_string()
}

fn debug_type_key(api_type: &ApiType) -> Option<String> {
    if let Some(name) = api_type.as_simple_name() {
        return Some(normalize_simple_type_key(name));
    }
    match api_type.complex_type() {
        Some("array") | Some("dictionary") | Some("LuaCustomTable") | Some("table") => {
            Some("table".to_string())
        }
        Some("union") => {
            let non_nil = api_type.non_nil_options();
            match non_nil.len() {
                1 => debug_type_key(&non_nil[0]),
                _ => Some("table".to_string()),
            }
        }
        Some("type") => api_type
            .child_type("value")
            .and_then(|value| debug_type_key(&value)),
        Some("literal") => match api_type.literal_kind() {
            Some("string") => Some("string".to_string()),
            Some("number") => Some("number".to_string()),
            Some("boolean") => Some("boolean".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn normalize_simple_type_key(name: &str) -> String {
    if name.starts_with("defines.") {
        return "string".to_string();
    }
    match name {
        "uint8" | "uint16" | "uint32" | "uint64" | "uint" | "int8" | "int16" | "int32"
        | "int64" | "int" | "float" | "double" | "number" | "MapTick" | "Tick"
        | "ItemStackIndex" | "ItemCountType" => "number".to_string(),
        "boolean" | "bool" => "boolean".to_string(),
        "string" | "LocalisedString" => "string".to_string(),
        other => other.to_string(),
    }
}
