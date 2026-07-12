use std::collections::{BTreeSet, HashMap, HashSet};

use proc_macro2::TokenStream;
use quote::quote;

use crate::generate::ident::{make_ident, sanitize_doc, to_pascal_case};
use crate::generate::types::{
    KnownTypes, lua_any_type, map_api_type, map_copy_field_type, map_field_type_unboxed,
    map_parameter_type, map_return_stub, map_return_type, return_stub_for_type, stub_expr,
};
use crate::schema::{Attribute, Class, Method, RuntimeApi};

type AttributePair<'a> = (&'a Attribute, &'a str);
type MethodPair<'a> = (&'a Method, &'a str);

/// Returns the fully-inherited attribute and method lists for `class` by walking
/// the `parent` chain. Parent members come first; child members override duplicates.
fn inherited_members<'a>(
    class: &'a Class,
    by_name: &'a HashMap<&'a str, &'a Class>,
) -> (Vec<AttributePair<'a>>, Vec<MethodPair<'a>>) {
    let mut attrs: Vec<(&Attribute, &str)> = Vec::new();
    let mut methods: Vec<(&Method, &str)> = Vec::new();

    let mut chain: Vec<&Class> = Vec::new();
    let mut current = class;
    loop {
        chain.push(current);
        match current.parent.as_deref().and_then(|p| by_name.get(p)) {
            Some(parent) => current = parent,
            None => break,
        }
    }
    chain.reverse();

    let mut seen_attrs: HashSet<&str> = HashSet::new();
    let mut seen_methods: HashSet<&str> = HashSet::new();

    for ancestor in chain {
        for attr in &ancestor.attributes {
            if seen_attrs.insert(attr.name.as_str()) {
                attrs.push((attr, ancestor.name.as_str()));
            }
        }
        for method in &ancestor.methods {
            if seen_methods.insert(method.name.as_str()) {
                methods.push((method, ancestor.name.as_str()));
            }
        }
    }

    (attrs, methods)
}

/// For each attribute on `class` (the *defining* class, not a subclass) that has
/// an inline `table` type, generate a named struct and return its identifier so
/// the parent class generator can reference it.
///
/// Returns `(struct_token_streams, attr_name -> struct_ident)`.
fn generate_inline_table_structs(
    class_name_str: &str,
    attrs: &[&Attribute],
    known: &KnownTypes<'_>,
) -> (Vec<TokenStream>, HashMap<String, proc_macro2::Ident>) {
    let mut structs: Vec<TokenStream> = Vec::new();
    let mut name_map: HashMap<String, proc_macro2::Ident> = HashMap::new();

    for attr in attrs {
        let Some(read_type) = &attr.read_type else {
            continue;
        };
        if read_type.complex_type() != Some("table") {
            continue;
        }
        let params = read_type.parameters();
        if params.is_empty() {
            continue;
        }

        let pascal_attr = to_pascal_case(&attr.name);
        let type_name_str = format!("{class_name_str}{pascal_attr}");
        let type_ident = make_ident(&type_name_str);

        let fields = params.iter().map(|(field_name, field_type, _optional)| {
            let ident = make_ident(field_name);
            // Use copy-compatible field types
            let ty = map_copy_field_type(field_type, known);
            quote! { pub #ident: #ty, }
        });

        structs.push(quote! {
            #[derive(Debug, Clone, Copy, PartialEq, Default)]
            pub struct #type_ident {
                #( #fields )*
            }
        });
        name_map.insert(attr.name.clone(), type_ident);
    }

    (structs, name_map)
}

pub fn class_names(api: &RuntimeApi) -> BTreeSet<String> {
    api.classes.iter().map(|class| class.name.clone()).collect()
}

fn method_rust_name(name: &str) -> proc_macro2::Ident {
    if name == "type" {
        make_ident("get_type")
    } else {
        make_ident(name)
    }
}

fn generate_method(
    method: &crate::schema::Method,
    known: &KnownTypes<'_>,
    params_struct: Option<&proc_macro2::Ident>,
) -> TokenStream {
    let name = method_rust_name(&method.name);
    let return_type = map_return_type(&method.return_values, known);
    let body = stub_expr(&map_return_stub(&method.return_values, known));

    let params: Vec<TokenStream> = if let Some(struct_ident) = params_struct {
        // takes_table method, single named params struct argument
        vec![quote!(params: #struct_ident)]
    } else {
        method
            .parameters
            .iter()
            .map(|parameter| {
                let param_name = make_ident(&parameter.name);
                let param_type = map_parameter_type(parameter, known);
                quote!( #param_name: #param_type )
            })
            .collect()
    };

    let doc: Option<String> = if method.description.is_empty() {
        None
    } else {
        Some(sanitize_doc(&method.description))
    };

    if let Some(description) = doc {
        quote! {
            #[doc = #description]
            #[allow(clippy::too_many_arguments, unused_variables)]
            pub fn #name(&self, #( #params ),* ) -> #return_type #body
        }
    } else {
        quote! {
            #[allow(clippy::too_many_arguments, unused_variables)]
            pub fn #name(&self, #( #params ),* ) -> #return_type #body
        }
    }
}

/// Generates a zero-argument `&self` method for a Factorio API attribute (property).
fn generate_attribute(
    attribute: &crate::schema::Attribute,
    known: &KnownTypes<'_>,
    reserved_names: &HashSet<String>,
    defining_class: &str,
    inline_types: &HashMap<String, proc_macro2::Ident>,
) -> Option<TokenStream> {
    let method_name = make_ident(&attribute.name);
    // Skip if there is already a real method with the same Rust name.
    if reserved_names.contains(&method_name.to_string()) {
        return None;
    }

    // Determine the return type and a matching stub body.
    let (return_type, body) = if let Some(type_ident) = inline_types.get(&attribute.name) {
        // Pre-generated inline-table struct - return it by value.
        (quote!(#type_ident), quote!({ Default::default() }))
    } else if attribute
        .read_type
        .as_ref()
        .is_some_and(|t| t.complex_type() == Some("table"))
    {
        // Inline table defined on an ancestor class - reference its named struct.
        let pascal = to_pascal_case(&attribute.name);
        let type_ident = make_ident(&format!("{defining_class}{pascal}"));
        (quote!(#type_ident), quote!({ Default::default() }))
    } else {
        let api_type_opt = attribute.read_type.as_ref();
        let ret = api_type_opt
            .map(|t| map_api_type(t, known))
            .unwrap_or_else(lua_any_type);
        let body = api_type_opt
            .map(|t| stub_expr(&return_stub_for_type(t, known)))
            .unwrap_or_else(|| quote!({ crate::LuaAny }));
        (ret, body)
    };

    let doc: Option<String> = if attribute.description.is_empty() {
        None
    } else {
        Some(sanitize_doc(&attribute.description))
    };

    if let Some(description) = doc {
        Some(quote! {
            #[doc = #description]
            pub fn #method_name(&self) -> #return_type #body
        })
    } else {
        Some(quote! {
            pub fn #method_name(&self) -> #return_type #body
        })
    }
}

type TakesTableMap = HashMap<(String, String), proc_macro2::Ident>;

fn build_takes_table_structs(
    api: &RuntimeApi,
    known: &KnownTypes<'_>,
) -> (Vec<TokenStream>, TakesTableMap) {
    let mut structs: Vec<TokenStream> = Vec::new();
    let mut map: TakesTableMap = HashMap::new();

    for class in &api.classes {
        for method in &class.methods {
            if !method.format.takes_table || method.parameters.is_empty() {
                continue;
            }

            let pascal_method = to_pascal_case(&method.name);
            let struct_name = format!("{}{}Params", class.name, pascal_method);
            let struct_ident = make_ident(&struct_name);

            let fields = method.parameters.iter().map(|p| {
                let ident = make_ident(&p.name);
                let ty = map_field_type_unboxed(&p.type_name, known);
                if p.description.is_empty() {
                    quote! { pub #ident: #ty, }
                } else {
                    let doc = sanitize_doc(&p.description);
                    quote! { #[doc = #doc] pub #ident: #ty, }
                }
            });

            let doc_attr = if method.description.is_empty() {
                quote!()
            } else {
                let d = sanitize_doc(&method.description);
                quote!(#[doc = #d])
            };

            structs.push(quote! {
                #doc_attr
                #[derive(Debug, Clone, Default)]
                pub struct #struct_ident {
                    #( #fields )*
                }
            });
            map.insert((class.name.clone(), method.name.clone()), struct_ident);
        }
    }

    (structs, map)
}

fn generate_class(
    class: &Class,
    known: &KnownTypes<'_>,
    by_name: &HashMap<&str, &Class>,
    takes_table_map: &TakesTableMap,
) -> TokenStream {
    let class_name = make_ident(&class.name);

    let (all_attrs, all_methods) = inherited_members(class, by_name);

    // Generate named structs for inline-table attributes defined directly on this class.
    let own_attrs: Vec<&Attribute> = all_attrs
        .iter()
        .filter(|(_, owner)| *owner == class.name.as_str())
        .map(|(attr, _)| *attr)
        .collect();
    let (inline_structs, inline_type_map) =
        generate_inline_table_structs(&class.name, &own_attrs, known);

    let mut reserved_names = HashSet::new();
    for (method, _) in &all_methods {
        reserved_names.insert(method_rust_name(&method.name).to_string());
    }

    let methods = all_methods.iter().map(|(method, defining_class)| {
        let params_struct = takes_table_map
            .get(&(defining_class.to_string(), method.name.clone()))
            .filter(|_| method.format.takes_table);
        generate_method(method, known, params_struct)
    });
    let attributes = all_attrs.iter().filter_map(|(attribute, defining_class)| {
        generate_attribute(
            attribute,
            known,
            &reserved_names,
            defining_class,
            &inline_type_map,
        )
    });

    let doc: Option<String> = if class.description.is_empty() {
        None
    } else {
        Some(sanitize_doc(&class.description))
    };

    let derive = quote!(#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]);

    if let Some(description) = doc {
        quote! {
            #( #inline_structs )*

            #[doc = #description]
            #derive
            pub struct #class_name;

            impl crate::LuaObject for #class_name {}

            impl From<#class_name> for crate::LuaAny {
                fn from(_: #class_name) -> Self { crate::LuaAny }
            }

            impl #class_name {
                #( #attributes )*
                #( #methods )*
            }
        }
    } else {
        quote! {
            #( #inline_structs )*

            #derive
            pub struct #class_name;

            impl crate::LuaObject for #class_name {}

            impl From<#class_name> for crate::LuaAny {
                fn from(_: #class_name) -> Self { crate::LuaAny }
            }

            impl #class_name {
                #( #attributes )*
                #( #methods )*
            }
        }
    }
}

pub fn generate_classes(api: &RuntimeApi, known: &KnownTypes<'_>) -> String {
    let by_name: HashMap<&str, &Class> = api.classes.iter().map(|c| (c.name.as_str(), c)).collect();

    let (takes_table_structs, takes_table_map) = build_takes_table_structs(api, known);

    let classes = api
        .classes
        .iter()
        .map(|class| generate_class(class, known, &by_name, &takes_table_map));

    let tokens = quote! {
        #( #takes_table_structs )*
        #( #classes )*
    };
    tokens.to_string()
}

pub fn generate_globals(api: &RuntimeApi, known: &KnownTypes<'_>) -> String {
    let globals = api.global_objects.iter().map(|global| {
        let global_name = make_ident(&global.name);
        let type_name = match global.type_name.as_simple_name() {
            Some(name) => {
                let ident = make_ident(name);
                quote!(crate::classes::#ident)
            }
            None => lua_any_type(),
        };
        quote! {
            pub static #global_name: std::sync::LazyLock<#type_name> =
                std::sync::LazyLock::new(#type_name::default);
        }
    });

    // Auxiliary globals from Factorio docs (not in `global_objects`).
    let auxiliary_globals = quote! {
        /// Persistent mod-local table. Serialized across save/load.
        ///
        /// See <https://lua-api.factorio.com/latest/auxiliary/storage.html>.
        pub static storage: std::sync::LazyLock<crate::LuaStorage> =
            std::sync::LazyLock::new(crate::LuaStorage::default);

        /// Deterministic table pretty-printer shipped as a Factorio global.
        ///
        /// See <https://lua-api.factorio.com/latest/auxiliary/libraries.html>.
        pub static serpent: std::sync::LazyLock<crate::Serpent> =
            std::sync::LazyLock::new(crate::Serpent::default);

        /// Standard Lua `math` library (Factorio-deterministic).
        pub static math: std::sync::LazyLock<crate::LuaMath> =
            std::sync::LazyLock::new(crate::LuaMath::default);

        /// Standard Lua `string` library (includes Factorio `pack` / `unpack`).
        pub static string: std::sync::LazyLock<crate::LuaStringLib> =
            std::sync::LazyLock::new(crate::LuaStringLib::default);

        /// Standard Lua `table` library.
        pub static table: std::sync::LazyLock<crate::LuaTableLib> =
            std::sync::LazyLock::new(crate::LuaTableLib::default);
    };

    let global_functions = api.global_functions.iter().map(|function| {
        let name = method_rust_name(&function.name);
        let return_type = map_return_type(&function.return_values, known);
        let body = stub_expr(&map_return_stub(&function.return_values, known));
        let params = function.parameters.iter().map(|parameter| {
            let param_name = global_function_param_ident(&parameter.name);
            let param_type = map_parameter_type(parameter, known);
            quote!( #param_name: #param_type )
        });

        quote! {
            #[allow(unused_variables)]
            pub fn #name( #( #params ),* ) -> #return_type #body
        }
    });

    let tokens = quote! {
        #( #globals )*
        #auxiliary_globals
        #( #global_functions )*
    };
    tokens.to_string()
}

/// Parameter names that would collide with generated global statics.
fn global_function_param_ident(name: &str) -> proc_macro2::Ident {
    const SHADOWING: &[&str] = &[
        "commands",
        "game",
        "helpers",
        "math",
        "prototypes",
        "rcon",
        "remote",
        "rendering",
        "script",
        "serpent",
        "settings",
        "storage",
        "string",
        "table",
    ];
    if SHADOWING.contains(&name) {
        make_ident(&format!("{name}_arg"))
    } else {
        make_ident(name)
    }
}
