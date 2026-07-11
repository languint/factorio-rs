use std::collections::{BTreeMap, BTreeSet, HashMap};

use proc_macro2::TokenStream;
use quote::quote;

use crate::generate::ident::{make_ident, sanitize_doc, sanitize_ident, to_pascal_case};
use crate::schema::{ApiType, RuntimeApi};

/// A generated unit enum for a Factorio homogeneous string-literal union.
#[derive(Debug, Clone)]
pub struct LiteralUnionEnum {
    pub name: String,
    pub doc: Option<String>,
    /// `(rust_variant, factorio_literal)` pairs in Factorio order.
    pub variants: Vec<(String, String)>,
}

/// Registry of all literal-union enums discovered in the API.
#[derive(Debug, Default)]
pub struct UnionRegistry {
    enums: Vec<LiteralUnionEnum>,
    /// Factorio literal list -> enum name (for dedup of anonymous unions).
    by_values: HashMap<Vec<String>, String>,
    names: BTreeSet<String>,
}

impl UnionRegistry {
    #[must_use]
    pub fn names(&self) -> &BTreeSet<String> {
        &self.names
    }

    #[must_use]
    pub fn enums(&self) -> &[LiteralUnionEnum] {
        &self.enums
    }

    /// Resolve a homogeneous string-literal union to its generated enum name.
    #[must_use]
    pub fn resolve(&self, api_type: &ApiType) -> Option<&str> {
        if !api_type.is_homog_string_literal_union() {
            return None;
        }
        let values = api_type.string_literal_values();
        self.by_values.get(&values).map(String::as_str)
    }

    fn insert(&mut self, name: String, doc: Option<String>, literals: Vec<String>) {
        if self.by_values.contains_key(&literals) {
            return;
        }

        let mut unique_name = name;
        if self.names.contains(&unique_name) {
            let mut suffix = 2u32;
            while self.names.contains(&format!("{unique_name}{suffix}")) {
                suffix = suffix.saturating_add(1);
            }
            unique_name = format!("{unique_name}{suffix}");
        }

        let variants = unique_variants(&literals);
        self.names.insert(unique_name.clone());
        self.by_values.insert(literals, unique_name.clone());
        self.enums.push(LiteralUnionEnum {
            name: unique_name,
            doc,
            variants,
        });
    }

    fn register_named(&mut self, name: &str, doc: &str, api_type: &ApiType) {
        let literals: Vec<String> = api_type.string_literal_values();
        if literals.is_empty() {
            return;
        }
        let doc = if doc.is_empty() {
            None
        } else {
            Some(sanitize_doc(doc))
        };
        // Prefer the named concept even if an anonymous union with the same
        // values was already registered under a different name.
        if let Some(existing) = self.by_values.get(&literals).cloned() {
            if existing == name {
                return;
            }
            // Re-key to the concept name and rename the enum entry.
            self.by_values.insert(literals, name.to_string());
            self.names.remove(&existing);
            self.names.insert(name.to_string());
            if let Some(entry) = self.enums.iter_mut().find(|e| e.name == existing) {
                entry.name = name.to_string();
                entry.doc = doc.or_else(|| entry.doc.clone());
            }
            return;
        }
        self.insert(name.to_string(), doc, literals);
    }

    fn register_anonymous(&mut self, owner: &str, member: &str, api_type: &ApiType) {
        let literals = api_type.string_literal_values();
        if literals.is_empty() {
            return;
        }
        if self.by_values.contains_key(&literals) {
            return;
        }
        let name = format!("{}{}", to_pascal_case(owner), to_pascal_case(member));
        self.insert(name, None, literals);
    }
}

/// Build the registry of all string-literal union enums in the API.
#[must_use]
pub fn collect_literal_unions(api: &RuntimeApi) -> UnionRegistry {
    let mut registry = UnionRegistry::default();

    // Named concepts first so they win dedup for identical anonymous unions.
    for concept in &api.concepts {
        if concept.type_name.is_homog_string_literal_union() {
            registry.register_named(&concept.name, &concept.description, &concept.type_name);
        }
    }

    for class in &api.classes {
        for attribute in &class.attributes {
            if let Some(ty) = attribute
                .read_type
                .as_ref()
                .or(attribute.write_type.as_ref())
            {
                walk_type_for_anonymous(&mut registry, &class.name, &attribute.name, ty);
            }
        }
        for method in &class.methods {
            for param in &method.parameters {
                let member = if param.name.is_empty() {
                    method.name.as_str()
                } else {
                    param.name.as_str()
                };
                walk_type_for_anonymous(&mut registry, &class.name, member, &param.type_name);
            }
            for ret in &method.return_values {
                walk_type_for_anonymous(&mut registry, &class.name, &method.name, &ret.type_name);
            }
        }
    }

    for event in &api.events {
        for param in &event.data {
            walk_type_for_anonymous(&mut registry, &event.name, &param.name, &param.type_name);
        }
    }

    for concept in &api.concepts {
        walk_concept_tables(&mut registry, concept.name.as_str(), &concept.type_name);
    }

    registry
}

fn walk_concept_tables(registry: &mut UnionRegistry, owner: &str, ty: &ApiType) {
    match ty.complex_type() {
        Some("table") => {
            for (field, field_ty, _) in ty.parameters() {
                walk_type_for_anonymous(registry, owner, &field, &field_ty);
            }
        }
        Some("union") => {
            for option in ty.options() {
                walk_concept_tables(registry, owner, &option);
            }
        }
        _ => {}
    }
}

fn walk_type_for_anonymous(registry: &mut UnionRegistry, owner: &str, member: &str, ty: &ApiType) {
    if ty.is_homog_string_literal_union() {
        registry.register_anonymous(owner, member, ty);
        return;
    }
    match ty.complex_type() {
        Some("array" | "dictionary" | "LuaCustomTable" | "LuaLazyLoadedValue" | "type") => {
            if let Some(inner) = ty.child_type("value").or_else(|| ty.child_type("key")) {
                walk_type_for_anonymous(registry, owner, member, &inner);
            }
        }
        Some("tuple") => {
            for (index, value) in ty.tuple_values().iter().enumerate() {
                walk_type_for_anonymous(registry, owner, &format!("{member}_{index}"), value);
            }
        }
        Some("table") => {
            for (field, field_ty, _) in ty.parameters() {
                walk_type_for_anonymous(registry, owner, &field, &field_ty);
            }
        }
        Some("union") => {
            for option in ty.non_nil_options() {
                walk_type_for_anonymous(registry, owner, member, &option);
            }
        }
        _ => {}
    }
}

fn unique_variants(literals: &[String]) -> Vec<(String, String)> {
    let mut used: BTreeMap<String, u32> = BTreeMap::new();
    literals
        .iter()
        .map(|literal| {
            let mut variant = literal_to_variant(literal);
            let count = used.entry(variant.clone()).or_insert(0);
            *count = count.saturating_add(1);
            if *count > 1 {
                variant = format!("{variant}_{count}");
            }
            (variant, literal.clone())
        })
        .collect()
}

fn literal_to_variant(literal: &str) -> String {
    match literal {
        "=" | "==" => return "Eq".to_string(),
        "≠" | "!=" => return "Ne".to_string(),
        ">" => return "Gt".to_string(),
        "<" => return "Lt".to_string(),
        "≥" | ">=" => return "Ge".to_string(),
        "≤" | "<=" => return "Le".to_string(),
        "*" => return "Mul".to_string(),
        "/" => return "Div".to_string(),
        "+" => return "Add".to_string(),
        "-" => return "Sub".to_string(),
        "%" => return "Mod".to_string(),
        "^" => return "Pow".to_string(),
        "<<" => return "Shl".to_string(),
        ">>" => return "Shr".to_string(),
        _ => {}
    }

    let normalized = literal.replace(['-', ' ', '/'], "_");
    let pascal = to_pascal_case(&normalized);
    let sanitized = sanitize_ident(&pascal);
    if sanitized.is_empty() || sanitized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("Value_{sanitized}")
    } else {
        sanitized
    }
}

/// Generate the `unions` module source.
#[must_use]
pub fn generate_unions(registry: &UnionRegistry) -> String {
    let header = "#![allow(nonstandard_style)]\n";
    let enums = registry.enums().iter().map(generate_enum);
    let lookup_arms = registry.enums().iter().flat_map(|enumeration| {
        enumeration.variants.iter().map(move |(variant, literal)| {
            let type_name = enumeration.name.as_str();
            let variant_name = variant.as_str();
            quote! { (#type_name, #variant_name) => Some(#literal), }
        })
    });

    let tokens = quote! {
        #( #enums )*

        /// Map a generated literal-union enum path (`Type`, `Variant`) to its Factorio string.
        #[must_use]
        pub fn literal_enum_variant_str(type_name: &str, variant: &str) -> Option<&'static str> {
            match (type_name, variant) {
                #( #lookup_arms )*
                _ => None,
            }
        }
    };
    format!("{header}{tokens}")
}

fn generate_enum(enumeration: &LiteralUnionEnum) -> TokenStream {
    let name = make_ident(&enumeration.name);
    let variants: Vec<_> = enumeration
        .variants
        .iter()
        .map(|(variant, _)| make_ident(variant))
        .collect();
    let first = variants
        .first()
        .cloned()
        .unwrap_or_else(|| make_ident("Unknown"));

    let as_str_arms = enumeration.variants.iter().map(|(variant, literal)| {
        let variant_ident = make_ident(variant);
        quote! { Self::#variant_ident => #literal, }
    });

    let doc_attr = enumeration.doc.as_ref().map(|d| quote!(#[doc = #d]));

    quote! {
        #doc_attr
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum #name {
            #( #variants, )*
        }

        impl #name {
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    #( #as_str_arms )*
                }
            }
        }

        impl Default for #name {
            fn default() -> Self {
                Self::#first
            }
        }

        impl PartialEq<&str> for #name {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        impl PartialEq<#name> for &str {
            fn eq(&self, other: &#name) -> bool {
                *self == other.as_str()
            }
        }

        impl From<#name> for crate::LuaAny {
            fn from(_: #name) -> Self {
                crate::LuaAny
            }
        }

        impl From<#name> for &'static str {
            fn from(value: #name) -> Self {
                value.as_str()
            }
        }
    }
}
