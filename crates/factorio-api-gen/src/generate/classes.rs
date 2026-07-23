use std::collections::{BTreeSet, HashMap, HashSet};

use proc_macro2::TokenStream;
use quote::quote;

use crate::generate::ident::{make_ident, sanitize_doc, to_pascal_case};
use crate::generate::types::{
    ClassOrStringPrefer, KnownTypes, lua_any_type, map_api_type, map_class_or_string_union,
    map_copy_field_type, map_field_type_unboxed, map_parameter_type, map_return_stub,
    map_return_type, return_stub_for_type, stub_expr,
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

        let fields = params.iter().map(|(field_name, field_type, optional)| {
            let ident = make_ident(field_name);
            // Use copy-compatible field types
            let base = map_copy_field_type(field_type, known);
            let ty = if *optional {
                quote!(Option<#base>)
            } else {
                base
            };
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
        let mut parameters = method.parameters.clone();
        parameters.sort_by_key(|parameter| parameter.order);
        parameters
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

/// Generates getter and/or setter methods for a Factorio API attribute.
///
/// - Readable attrs -> zero-arg getter (`caption()` -> property read in Lua).
/// - Writable attrs -> `set_<name>` (or `write_<name>` if `set_*` collides with a
///   real Factorio method). Codegen rewrites one-arg setters to property assign.
/// - Write-only attrs get **no** getter (avoids fake `LuaAny` getters).
fn generate_attribute(
    attribute: &crate::schema::Attribute,
    known: &KnownTypes<'_>,
    reserved_names: &mut HashSet<String>,
    defining_class: &str,
    inline_types: &HashMap<String, proc_macro2::Ident>,
) -> Vec<TokenStream> {
    let mut methods = Vec::new();

    let has_read = attribute.read_type.is_some() || inline_types.contains_key(&attribute.name);
    let has_write = attribute.write_type.is_some();

    if has_read {
        let method_name = make_ident(&attribute.name);
        let method_key = method_name.to_string();
        if reserved_names.insert(method_key) {
            let getter = if let Some(type_ident) = inline_types.get(&attribute.name) {
                Some((quote!(#type_ident), quote!({ Default::default() })))
            } else if attribute
                .read_type
                .as_ref()
                .is_some_and(|t| t.complex_type() == Some("table"))
            {
                let pascal = to_pascal_case(&attribute.name);
                let type_ident = make_ident(&format!("{defining_class}{pascal}"));
                Some((quote!(#type_ident), quote!({ Default::default() })))
            } else {
                attribute.read_type.as_ref().map(|read_ty| {
                    (
                        map_api_type(read_ty, known),
                        stub_expr(&return_stub_for_type(read_ty, known)),
                    )
                })
            };

            if let Some((return_type, body)) = getter {
                let doc: Option<String> = if attribute.description.is_empty() {
                    None
                } else {
                    Some(sanitize_doc(&attribute.description))
                };
                methods.push(if let Some(description) = doc {
                    quote! {
                        #[doc = #description]
                        pub fn #method_name(&self) -> #return_type #body
                    }
                } else {
                    quote! {
                        pub fn #method_name(&self) -> #return_type #body
                    }
                });
            } else {
                // Roll back reservation if we did not emit a getter.
                reserved_names.remove(&method_name.to_string());
            }
        }
    }

    if has_write {
        let Some(write_ty) = attribute.write_type.as_ref() else {
            return methods;
        };
        let set_name = format!("set_{}", attribute.name);
        let write_name = format!("write_{}", attribute.name);
        let setter_name_str = if reserved_names.contains(&set_name) {
            write_name
        } else {
            set_name
        };
        if !reserved_names.insert(setter_name_str.clone()) {
            return methods;
        }

        let setter_ident = make_ident(&setter_name_str);
        // Prefer the same inline table struct the getter uses (e.g. walking_state),
        // including when the attr is inherited from `defining_class`.
        let value_ty = if let Some(type_ident) = inline_types.get(&attribute.name) {
            quote!(#type_ident)
        } else if write_ty.complex_type() == Some("table") {
            let pascal = to_pascal_case(&attribute.name);
            let type_ident = make_ident(&format!("{defining_class}{pascal}"));
            quote!(#type_ident)
        } else {
            map_setter_value_type(write_ty, known)
        };
        let prop = attribute.name.as_str();
        let doc = if attribute.description.is_empty() {
            format!("Set the `{prop}` attribute (Lua: `self.{prop} = value`).")
        } else {
            format!(
                "Set the `{prop}` attribute (Lua: `self.{prop} = value`).\n\n{}",
                sanitize_doc(&attribute.description)
            )
        };

        methods.push(quote! {
            #[doc = #doc]
            #[allow(unused_variables)]
            pub fn #setter_ident(&self, value: #value_ty) {}
        });
    }

    methods
}

/// Rust parameter type for attribute writers (`impl Into<LocalisedString>`, etc.).
fn map_setter_value_type(api_type: &crate::schema::ApiType, known: &KnownTypes<'_>) -> TokenStream {
    let is_localised = matches!(
        api_type.as_simple_name(),
        Some("LocalisedString" | "LuaLazyLoadedValueLocalisedString")
    );
    if is_localised {
        return quote!(impl Into<crate::LocalisedString>);
    }
    if api_type.complex_type() == Some("union") {
        let options = api_type.options();
        let non_nil: Vec<_> = options
            .iter()
            .filter(|o| o.as_simple_name() != Some("nil"))
            .collect();
        if let Some(ty) = map_class_or_string_union(&non_nil, known, ClassOrStringPrefer::String) {
            return ty;
        }
    }
    map_api_type(api_type, known)
}

/// Lookup used by factorio-codegen: attribute setter method -> Lua property name.
pub fn generate_attribute_setter_lookup(api: &RuntimeApi) -> String {
    use std::collections::BTreeMap;

    let by_name: HashMap<&str, &Class> = api.classes.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut method_names: BTreeSet<String> = BTreeSet::new();
    for class in &api.classes {
        for method in &class.methods {
            method_names.insert(method.name.clone());
        }
    }

    // setter_method -> property (stable order)
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    let mut readable_attrs: BTreeSet<String> = BTreeSet::new();
    for class in &api.classes {
        let (attrs, _) = inherited_members(class, &by_name);
        for (attribute, _) in attrs {
            if attribute.read_type.is_some() {
                readable_attrs.insert(attribute.name.clone());
            }
            let Some(_) = attribute.write_type.as_ref() else {
                continue;
            };
            let set_name = format!("set_{}", attribute.name);
            let setter = if method_names.contains(&set_name) {
                format!("write_{}", attribute.name)
            } else {
                set_name
            };
            map.insert(setter, attribute.name.clone());
        }
    }

    let arms = map.iter().map(|(setter, prop)| {
        let setter_lit = setter.as_str();
        let prop_lit = prop.as_str();
        quote! { #setter_lit => Some(#prop_lit), }
    });
    let attr_arms = readable_attrs.iter().map(|name| {
        let lit = name.as_str();
        quote! { #lit => true, }
    });
    let method_arms = method_names.iter().map(|name| {
        let lit = name.as_str();
        quote! { #lit => true, }
    });

    let tokens = quote! {
        /// Map Factorio attribute setter stubs (`set_caption`, `write_driving`, ...)
        /// to the Lua property name. Real Factorio `set_*` **methods** are absent
        /// from this table so codegen keeps them as method calls.
        #[must_use]
        pub fn attribute_property_for_setter(method: &str) -> Option<&'static str> {
            match method {
                #( #arms )*
                _ => None,
            }
        }

        /// Readable Factorio attributes (`entity.surface`, `game.tick`, ...).
        /// Zero-arg Rust calls with these names emit property reads, not invocations.
        #[must_use]
        pub fn is_factorio_attribute_read(method: &str) -> bool {
            match method {
                #( #attr_arms )*
                _ => false,
            }
        }

        /// Factorio `LuaObject` methods (`die`, `clear`, ...). Unknown names are treated
        /// as user metatable methods and use `:` so `self` is passed.
        #[must_use]
        pub fn is_factorio_method(method: &str) -> bool {
            match method {
                #( #method_arms )*
                _ => false,
            }
        }
    };
    tokens.to_string()
}

type TakesTableMap = HashMap<(String, String), proc_macro2::Ident>;

fn takes_table_field(
    parameter: &crate::schema::Parameter,
    known: &KnownTypes<'_>,
    optional: bool,
) -> TokenStream {
    let ident = make_ident(&parameter.name);
    let base = map_field_type_unboxed(&parameter.type_name, known);
    let ty = if optional {
        quote!(Option<#base>)
    } else {
        base
    };
    if parameter.description.is_empty() {
        quote! { pub #ident: #ty, }
    } else {
        let doc = sanitize_doc(&parameter.description);
        quote! { #[doc = #doc] pub #ident: #ty, }
    }
}

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

            let mut seen_names = HashSet::new();
            let mut fields: Vec<TokenStream> = Vec::new();

            for p in &method.parameters {
                seen_names.insert(p.name.clone());
                fields.push(takes_table_field(p, known, p.optional));
            }

            // Flatten variant groups (e.g. LuaGuiElement.add `direction`) into the
            // same params struct so callers can set create-time-only fields.
            // Always `Option<_>`: applicability depends on `type`.
            for group in &method.variant_parameter_groups {
                for p in &group.parameters {
                    if !seen_names.insert(p.name.clone()) {
                        continue;
                    }
                    fields.push(takes_table_field(p, known, true));
                }
            }

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

    let methods = all_methods.iter().map(|&(method, defining_class)| {
        let params_struct = takes_table_map
            .get(&(defining_class.to_string(), method.name.clone()))
            .filter(|_| method.format.takes_table);
        generate_method(method, known, params_struct)
    });
    let mut attribute_methods = Vec::new();
    for (attribute, defining_class) in &all_attrs {
        attribute_methods.extend(generate_attribute(
            attribute,
            known,
            &mut reserved_names,
            defining_class,
            &inline_type_map,
        ));
    }
    let attributes = attribute_methods;

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
        let mut parameters = function.parameters.clone();
        parameters.sort_by_key(|parameter| parameter.order);
        let params = parameters.iter().map(|parameter| {
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
