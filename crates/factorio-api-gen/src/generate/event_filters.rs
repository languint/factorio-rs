use quote::quote;

use crate::generate::ident::{make_ident, sanitize_ident};
use crate::schema::{Concept, RuntimeApi};

fn literal_value(api_type: &crate::schema::ApiType) -> Option<String> {
    let value = api_type.0.get("value")?;
    match value {
        serde_json::Value::String(string) => Some(string.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn filter_union_literals(api_type: &crate::schema::ApiType) -> Vec<String> {
    if api_type.complex_type() != Some("union") {
        return Vec::new();
    }

    api_type
        .options()
        .iter()
        .filter_map(literal_value)
        .collect()
}

fn literal_method_name(value: &str) -> String {
    sanitize_ident(value)
}

fn value_field_for_filter(filter: &str) -> Option<&'static str> {
    match filter {
        "type" => Some("type"),
        "name" => Some("name"),
        "ghost_type" => Some("type"),
        "ghost_name" => Some("name"),
        "force" => Some("force"),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FilterMethodSpec {
    pub filter: String,
    pub value_field: Option<String>,
    pub arg_count: usize,
}

fn method_name_for_spec(spec: &FilterMethodSpec) -> String {
    if spec.arg_count == 0 {
        return literal_method_name(&spec.filter);
    }
    if spec.filter == "type" {
        return "type_".to_string();
    }
    sanitize_ident(&spec.filter)
}

fn collect_filter_methods(concept: &Concept) -> Vec<FilterMethodSpec> {
    let Some(table) = concept.type_name.0.as_object() else {
        return Vec::new();
    };
    if table.get("complex_type").and_then(|value| value.as_str()) != Some("table") {
        return Vec::new();
    }

    let mut methods = Vec::new();
    let variant_groups = table
        .get("variant_parameter_groups")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let variant_group_names: std::collections::BTreeSet<String> = variant_groups
        .iter()
        .filter_map(|group| group.get("name").and_then(|value| value.as_str()))
        .map(str::to_string)
        .collect();

    if let Some(parameters) = table.get("parameters").and_then(|value| value.as_array()) {
        for parameter in parameters {
            if parameter.get("name").and_then(|value| value.as_str()) != Some("filter") {
                continue;
            }
            let Some(filter_type) = parameter
                .get("type")
                .map(|value| crate::schema::ApiType(value.clone()))
            else {
                continue;
            };
            for literal in filter_union_literals(&filter_type) {
                if value_field_for_filter(&literal).is_some() {
                    continue;
                }
                if variant_group_names.contains(&literal) {
                    continue;
                }
                methods.push(FilterMethodSpec {
                    filter: literal,
                    value_field: None,
                    arg_count: 0,
                });
            }
        }
    }

    for group in &variant_groups {
        let Some(group_name) = group.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        let parameters = group
            .get("parameters")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        // Nested choose-elem groups: `{ filter = "...", elem_filters = { ... } }`.
        if parameters.iter().any(is_nested_elem_filters_param) {
            if parameters.len() == 1 && is_nested_elem_filters_param(&parameters[0]) {
                methods.push(FilterMethodSpec {
                    filter: group_name.to_string(),
                    value_field: Some("elem_filters".to_string()),
                    arg_count: 1,
                });
            }
            continue;
        }
        let arg_count = parameters.len().max(1);
        let value_field = value_field_for_filter(group_name)
            .map(str::to_string)
            .or_else(|| {
                if parameters.len() == 1 {
                    parameters[0]
                        .get("name")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                } else {
                    None
                }
            });
        methods.push(FilterMethodSpec {
            filter: group_name.to_string(),
            value_field,
            arg_count,
        });
    }

    let mut seen = std::collections::BTreeSet::new();
    methods.retain(|method| seen.insert(method_name_for_spec(method)));
    methods
}

fn is_nested_elem_filters_param(parameter: &serde_json::Value) -> bool {
    if parameter.get("name").and_then(|value| value.as_str()) == Some("elem_filters") {
        return true;
    }
    let Some(ty) = parameter.get("type") else {
        return false;
    };
    if ty.as_str() == Some("PrototypeFilter") {
        return true;
    }
    if ty.get("complex_type").and_then(|value| value.as_str()) == Some("array") {
        let value = ty.get("value");
        if value.and_then(|v| v.as_str()) == Some("PrototypeFilter") {
            return true;
        }
        if value
            .and_then(|v| v.get("complex_type"))
            .and_then(|v| v.as_str())
            == Some("type")
            && value.and_then(|v| v.get("value")).and_then(|v| v.as_str())
                == Some("PrototypeFilter")
        {
            return true;
        }
    }
    false
}

/// Concept names used as [`PrototypeFilter`] array arms (choose-elem filters).
pub fn prototype_filter_concept_names(api: &RuntimeApi) -> std::collections::BTreeSet<String> {
    let Some(concept) = api.concepts.iter().find(|c| c.name == "PrototypeFilter") else {
        return std::collections::BTreeSet::new();
    };
    let Some(value) = concept.type_name.child_type("value") else {
        return std::collections::BTreeSet::new();
    };
    value
        .non_nil_options()
        .into_iter()
        .filter_map(|arm| {
            let arm = if arm.complex_type() == Some("type") {
                arm.child_type("value").unwrap_or(arm)
            } else {
                arm
            };
            arm.as_simple_name().map(str::to_string)
        })
        .collect()
}

fn emit_filter_type_builders(
    filter_names: &std::collections::BTreeSet<String>,
    api: &RuntimeApi,
    entry_ty: &str,
) -> (Vec<proc_macro2::TokenStream>, Vec<FilterMethodSpec>) {
    let entry_ident = make_ident(entry_ty);
    let mut all_methods = Vec::new();
    let mut filter_types = Vec::new();

    for filter_name in filter_names {
        let Some(concept) = api
            .concepts
            .iter()
            .find(|concept| concept.name == *filter_name)
        else {
            continue;
        };

        let methods = collect_filter_methods(concept);
        all_methods.extend(methods.iter().cloned());

        let type_ident = make_ident(filter_name);
        let method_items = methods.iter().map(|method| {
            let method_ident = make_ident(&method_name_for_spec(method));
            if method.arg_count == 0 {
                quote! {
                    pub const fn #method_ident() -> #entry_ident { #entry_ident }
                }
            } else if method.value_field.as_deref() == Some("elem_filters") {
                quote! {
                    pub const fn #method_ident(_elem_filters: &[#entry_ident]) -> #entry_ident {
                        #entry_ident
                    }
                }
            } else if method.arg_count == 1 {
                quote! {
                    pub const fn #method_ident(_value: &str) -> #entry_ident { #entry_ident }
                }
            } else {
                quote! {
                    pub fn #method_ident(_comparison: &str, _value: f64) -> #entry_ident { #entry_ident }
                }
            }
        });

        filter_types.push(quote! {
            #[derive(Copy, Clone, Debug, PartialEq, Eq)]
            pub struct #type_ident;

            impl #type_ident {
                #( #method_items )*
            }
        });
    }

    (filter_types, all_methods)
}

fn lookup_arms_for_methods(all_methods: &[FilterMethodSpec]) -> Vec<proc_macro2::TokenStream> {
    let mut methods = all_methods.to_vec();
    methods.sort_by(|left, right| left.filter.cmp(&right.filter));
    methods.dedup_by(|left, right| {
        left.filter == right.filter
            && left.value_field == right.value_field
            && left.arg_count == right.arg_count
    });

    methods
        .iter()
        .flat_map(|method| {
            let filter = &method.filter;
            let arg_count = method.arg_count;
            let method_names: Vec<String> = if arg_count == 0 {
                vec![literal_method_name(filter)]
            } else if filter == "type" {
                vec!["type_".to_string()]
            } else {
                vec![sanitize_ident(filter)]
            };

            method_names.into_iter().filter_map(move |method_name| {
                match (&method.value_field, arg_count) {
                    (Some(field), 1) => {
                        let field = field.as_str();
                        Some(quote! {
                            #method_name => Some(FilterMethodSpec {
                                filter: #filter,
                                value_field: Some(#field),
                                arg_count: 1,
                            })
                        })
                    }
                    (None, 2) => Some(quote! {
                        #method_name => Some(FilterMethodSpec {
                            filter: #filter,
                            value_field: None,
                            arg_count: 2,
                        })
                    }),
                    (None, 0) => Some(quote! {
                        #method_name => Some(FilterMethodSpec {
                            filter: #filter,
                            value_field: None,
                            arg_count: 0,
                        })
                    }),
                    _ => None,
                }
            })
        })
        .collect()
}

pub fn generate_event_filters(api: &RuntimeApi) -> String {
    let event_filter_names: std::collections::BTreeSet<String> = api
        .events
        .iter()
        .filter_map(|event| event.filter.clone())
        .collect();
    let prototype_filter_names = prototype_filter_concept_names(api);

    let (event_types, event_methods) =
        emit_filter_type_builders(&event_filter_names, api, "EventFilterEntry");
    let (proto_types, proto_methods) =
        emit_filter_type_builders(&prototype_filter_names, api, "PrototypeFilterEntry");

    let mut all_methods = event_methods;
    all_methods.extend(proto_methods);
    let lookup_arms = lookup_arms_for_methods(&all_methods);

    let types_tokens = quote! {
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        pub struct EventFilterEntry;

        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        pub struct PrototypeFilterEntry;

        #( #event_types )*
        #( #proto_types )*

        pub struct FilterMethodSpec {
            pub filter: &'static str,
            pub value_field: Option<&'static str>,
            pub arg_count: usize,
        }

        pub fn filter_method_spec(method: &str) -> Option<FilterMethodSpec> {
            match method {
                #( #lookup_arms, )*
                _ => None,
            }
        }
    };

    types_tokens.to_string()
}

pub fn generate_event_data(
    api: &RuntimeApi,
    known: &crate::generate::types::KnownTypes<'_>,
) -> String {
    use crate::generate::events::event_rust_name;
    use crate::generate::types::map_copy_field_type;

    let event_data = api.events.iter().map(|event| {
        let rust_name = make_ident(&format!("{}Event", event_rust_name(&event.name)));
        let fields = event.data.iter().map(|parameter| {
            let field_name = make_ident(&parameter.name);
            let field_type = map_copy_field_type(&parameter.type_name, known);
            quote! {
                pub #field_name: #field_type,
            }
        });

        let doc = if event.description.is_empty() {
            None
        } else {
            Some(event.description.as_str())
        };

        if let Some(description) = doc {
            quote! {
                #[doc = #description]
                #[derive(Debug, Clone, Copy, PartialEq, Default)]
                pub struct #rust_name {
                    #( #fields )*
                }
            }
        } else {
            quote! {
                #[derive(Debug, Clone, Copy, PartialEq, Default)]
                pub struct #rust_name {
                    #( #fields )*
                }
            }
        }
    });

    quote! { #( #event_data )* }.to_string()
}
