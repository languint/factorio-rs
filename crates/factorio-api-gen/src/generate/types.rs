use std::collections::BTreeSet;

use crate::generate::ident::make_ident;
use proc_macro2::TokenStream;
use quote::quote;

use crate::schema::ApiType;

/// All named Rust types the generator knows about, split by module so
/// the emitted paths are correct.
pub struct KnownTypes<'a> {
    /// `crate::classes::*` - emitted as `Box<T>` in field position to break cycles.
    pub classes: &'a BTreeSet<String>,
    /// `crate::concepts::*` - emitted as `T` (value types, no boxing needed).
    pub concepts: &'a BTreeSet<String>,
}

/// Opaque placeholder for complex Factorio Lua API values.
pub fn lua_any_type() -> TokenStream {
    quote!(crate::LuaAny)
}

pub enum ReturnStub {
    Unit,
    Bool,
    /// Integer stub - any `{ 0 }` will satisfy `u8`/`u16`/`u32`/`u64`/`i8`/`i16`/`i32`/`i64`
    /// through type inference from the function's declared return type.
    Int,
    /// Float stub - `{ 0.0 }` satisfies both `f32` and `f64` through inference.
    Number,
    Str,
    LuaAny,
    Default,
    Option(Box<ReturnStub>),
    Vec(Box<ReturnStub>),
    Tuple(Vec<ReturnStub>),
}

pub fn return_stub_for_type(api_type: &ApiType, known: &KnownTypes<'_>) -> ReturnStub {
    if let Some(name) = api_type.as_simple_name() {
        return match name {
            "boolean" => ReturnStub::Bool,
            "string" | "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => ReturnStub::Str,
            // Exact integer types - stub with `{ 0 }` (inferred by Rust to the return type).
            "uint8" | "uint16" | "uint32" | "uint64" | "uint" | "int8" | "int16" | "int32"
            | "int64" | "int" | "MapTick" | "Tick" | "ItemStackIndex" | "ItemCountType"
            | "InventoryIndex" => ReturnStub::Int,
            // Float types - stub with `{ 0.0 }`.
            "number" | "float" | "double" => ReturnStub::Number,
            "nil" | "void" => ReturnStub::Unit,
            other if other.starts_with("defines.") => ReturnStub::Str,
            other if known.classes.contains(other) || known.concepts.contains(other) => {
                ReturnStub::Default
            }
            _ => ReturnStub::LuaAny,
        };
    }

    match api_type.complex_type() {
        Some("array") => ReturnStub::Vec(Box::new(
            api_type
                .child_type("value")
                .map(|value| return_stub_for_type(&value, known))
                .unwrap_or(ReturnStub::LuaAny),
        )),
        Some("dictionary") | Some("LuaCustomTable") => {
            if api_type
                .child_type("key")
                .is_some_and(|k| is_string_key(&k))
            {
                ReturnStub::Default // HashMap::default() = HashMap::new()
            } else {
                ReturnStub::LuaAny
            }
        }
        Some("union") => {
            let options = api_type.options();
            let non_nil: Vec<_> = options
                .iter()
                .filter(|o| o.as_simple_name() != Some("nil"))
                .collect();
            let has_nil = options.len() > non_nil.len();
            match non_nil.len() {
                0 => ReturnStub::Unit,
                1 => {
                    let inner = return_stub_for_type(non_nil[0], known);
                    if has_nil {
                        ReturnStub::Option(Box::new(inner))
                    } else {
                        inner
                    }
                }
                _ => {
                    if all_same_literal_kind(&non_nil) {
                        let inner = match non_nil[0].literal_kind() {
                            Some("string") => ReturnStub::Str,
                            Some("number") => ReturnStub::Number,
                            Some("boolean") => ReturnStub::Bool,
                            _ => return ReturnStub::LuaAny,
                        };
                        if has_nil {
                            ReturnStub::Option(Box::new(inner))
                        } else {
                            inner
                        }
                    } else {
                        ReturnStub::LuaAny
                    }
                }
            }
        }
        Some("tuple") => {
            let values = api_type.tuple_values();
            if values.is_empty() {
                ReturnStub::LuaAny
            } else {
                ReturnStub::Tuple(
                    values
                        .iter()
                        .map(|v| return_stub_for_type(v, known))
                        .collect(),
                )
            }
        }
        Some("literal") => match api_type.literal_kind() {
            Some("string") => ReturnStub::Str,
            Some("number") => ReturnStub::Int,
            Some("boolean") => ReturnStub::Bool,
            _ => ReturnStub::LuaAny,
        },
        Some("type") => api_type
            .child_type("value")
            .map(|value| return_stub_for_type(&value, known))
            .unwrap_or(ReturnStub::LuaAny),
        Some("LuaLazyLoadedValue") => api_type
            .child_type("value")
            .map(|value| return_stub_for_type(&value, known))
            .unwrap_or(ReturnStub::LuaAny),
        _ => ReturnStub::LuaAny,
    }
}

/// Returns true when every element of `opts` is a `literal` of the same kind
/// (all strings, all numbers, or all booleans). Used to collapse homogeneous
/// literal unions to a single primitive type instead of `LuaAny`.
fn all_same_literal_kind(opts: &[&ApiType]) -> bool {
    let Some(first) = opts.first() else {
        return false;
    };
    let first_kind = first.literal_kind();
    first_kind.is_some() && opts[1..].iter().all(|o| o.literal_kind() == first_kind)
}

/// Returns true if `api_type` maps to a Rust type that implements `Eq + Hash`
/// and can therefore be used as a `HashMap` key.
fn is_string_key(api_type: &ApiType) -> bool {
    match api_type.as_simple_name() {
        Some("string" | "LocalisedString" | "LuaLazyLoadedValueLocalisedString") => true,
        Some(name) if name.starts_with("defines.") => true,
        _ => false,
    }
}

pub fn stub_expr(stub: &ReturnStub) -> TokenStream {
    match stub {
        ReturnStub::Unit => quote!({}),
        ReturnStub::Bool => quote!({ false }),
        ReturnStub::Int => quote!({ 0 }),
        ReturnStub::Number => quote!({ 0.0 }),
        ReturnStub::Str => quote!({ "" }),
        ReturnStub::LuaAny => quote!({ crate::LuaAny }),
        ReturnStub::Default => quote!({ Default::default() }),
        ReturnStub::Option(inner) => {
            let _ = inner;
            quote!({ None })
        }
        ReturnStub::Vec(inner) => {
            let _ = inner;
            quote!({ Vec::new() })
        }
        ReturnStub::Tuple(items) => {
            let values: Vec<_> = items
                .iter()
                .map(|item| match item {
                    ReturnStub::Unit => quote!(()),
                    ReturnStub::Bool => quote!(false),
                    ReturnStub::Int => quote!(0),
                    ReturnStub::Number => quote!(0.0),
                    ReturnStub::Str => quote!(""),
                    ReturnStub::LuaAny => quote!(crate::LuaAny),
                    ReturnStub::Default => quote!(Default::default()),
                    ReturnStub::Option(_) => quote!(None),
                    ReturnStub::Vec(_) => quote!(Vec::new()),
                    ReturnStub::Tuple(_) => quote!(()),
                })
                .collect();
            quote!({ (#(#values),*) })
        }
    }
}

pub fn map_api_type(api_type: &ApiType, known: &KnownTypes<'_>) -> TokenStream {
    if let Some(name) = api_type.as_simple_name() {
        return map_simple_type(name, known);
    }

    match api_type.complex_type() {
        Some("array") => {
            let inner = api_type
                .child_type("value")
                .map(|value| map_api_type(&value, known))
                .unwrap_or_else(lua_any_type);
            quote!(Vec<#inner>)
        }
        Some("dictionary") | Some("LuaCustomTable") => {
            map_dict_type(api_type, known, map_field_type)
        }
        Some("union") => map_union_type(api_type, known, map_api_type),
        Some("type") => api_type
            .child_type("value")
            .map(|value| map_api_type(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("tuple") => map_tuple_type(api_type, known, map_api_type),
        Some("literal") => map_literal_type(api_type),
        Some("function") | Some("LuaStruct") => lua_any_type(),
        Some("LuaLazyLoadedValue") => api_type
            .child_type("value")
            .map(|value| map_api_type(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("table") => lua_any_type(),
        _ => lua_any_type(),
    }
}

/// Maps a Factorio numeric API type name to the most precise Rust numeric type.
///
/// Integer types preserve their exact width (e.g. `uint16` → `u16`) so that
/// callers get useful range information and IDE diagnostics. Float types use the
/// appropriate float (`float` → `f32`, `double`/`number` → `f64`).
///
/// All of these are `Copy`, so the mapping is safe for every context.
fn map_numeric_type(name: &str) -> TokenStream {
    match name {
        "uint8" => quote!(u8),
        "uint16" => quote!(u16),
        "uint" | "uint32" => quote!(u32),
        "uint64" => quote!(u64),
        "int8" => quote!(i8),
        "int16" => quote!(i16),
        "int" | "int32" => quote!(i32),
        "int64" => quote!(i64),
        "float" => quote!(f32),
        "double" | "number" => quote!(f64),
        "MapTick" | "Tick" => quote!(u32),
        "ItemStackIndex" | "InventoryIndex" => quote!(u16),
        "ItemCountType" => quote!(u32),
        _ => unreachable!("map_numeric_type called with non-numeric name: {name}"),
    }
}

/// Returns `true` for all Factorio numeric type names that should be treated as integers.
fn is_integer_api_type(name: &str) -> bool {
    matches!(
        name,
        "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "uint"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "int"
            | "MapTick"
            | "Tick"
            | "ItemStackIndex"
            | "ItemCountType"
            | "InventoryIndex"
    )
}

fn map_simple_type(name: &str, known: &KnownTypes<'_>) -> TokenStream {
    match name {
        "string" | "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => quote!(&str),
        "boolean" => quote!(bool),
        "nil" | "void" => quote!(()),
        n if is_integer_api_type(n) || matches!(n, "float" | "double" | "number") => {
            map_numeric_type(n)
        }
        other if known.classes.contains(other) => {
            let ident = make_ident(other);
            quote!(crate::classes::#ident)
        }
        other if known.concepts.contains(other) => {
            let ident = make_ident(other);
            quote!(crate::concepts::#ident)
        }
        other if other.starts_with("defines.") => quote!(&str),
        _ => lua_any_type(),
    }
}

/// Like [`map_api_type`] but for struct fields: uses owned `String` instead of `&str`
/// so fields don't require lifetime parameters.
pub fn map_field_type(api_type: &ApiType, known: &KnownTypes<'_>) -> TokenStream {
    if let Some(name) = api_type.as_simple_name() {
        return map_simple_field_type(name, known);
    }

    match api_type.complex_type() {
        Some("array") => {
            let inner = api_type
                .child_type("value")
                .map(|value| map_field_type(&value, known))
                .unwrap_or_else(lua_any_type);
            quote!(Vec<#inner>)
        }
        Some("dictionary") | Some("LuaCustomTable") => {
            map_dict_type(api_type, known, map_field_type)
        }
        Some("union") => map_union_type(api_type, known, map_field_type),
        Some("type") => api_type
            .child_type("value")
            .map(|value| map_field_type(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("tuple") => map_tuple_type(api_type, known, map_field_type),
        Some("literal") => map_literal_field_type(api_type),
        Some("function") | Some("LuaStruct") => lua_any_type(),
        Some("LuaLazyLoadedValue") => api_type
            .child_type("value")
            .map(|value| map_field_type(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("table") => lua_any_type(),
        _ => lua_any_type(),
    }
}

// ── Shared helpers for complex type variants ──────────────────────────────

/// Maps `dictionary` / `LuaCustomTable` to `HashMap<String, V>` when the key is
/// a string type, otherwise falls back to `LuaAny`.
fn map_dict_type(
    api_type: &ApiType,
    known: &KnownTypes<'_>,
    map_inner: fn(&ApiType, &KnownTypes<'_>) -> TokenStream,
) -> TokenStream {
    let Some(key) = api_type.child_type("key") else {
        return lua_any_type();
    };
    if !is_string_key(&key) {
        return lua_any_type();
    }
    let value = api_type
        .child_type("value")
        .map(|v| map_inner(&v, known))
        .unwrap_or_else(lua_any_type);
    quote!(std::collections::HashMap<String, #value>)
}

/// Maps a `union` type:
/// - Single non-nil arm → that arm's type
/// - Single non-nil arm + nil(s) → `Option<T>`
/// - Multiple non-nil arms that are all literals of the same kind → collapsed primitive
/// - Multiple non-nil arms of different/complex types → `LuaAny`
fn map_union_type(
    api_type: &ApiType,
    known: &KnownTypes<'_>,
    map_inner: fn(&ApiType, &KnownTypes<'_>) -> TokenStream,
) -> TokenStream {
    let options = api_type.options();
    let non_nil: Vec<_> = options
        .iter()
        .filter(|o| o.as_simple_name() != Some("nil"))
        .collect();
    let has_nil = options.len() > non_nil.len();
    match non_nil.len() {
        0 => quote!(()),
        1 => {
            let inner = map_inner(non_nil[0], known);
            if has_nil {
                quote!(Option<#inner>)
            } else {
                inner
            }
        }
        _ => {
            // All options are literals of the same kind - delegate through `map_inner`
            // so the context (`&str` vs `String`) is preserved automatically.
            if all_same_literal_kind(&non_nil) {
                let ty = map_inner(non_nil[0], known);
                if has_nil { quote!(Option<#ty>) } else { ty }
            } else {
                lua_any_type()
            }
        }
    }
}

/// Maps a `tuple` type to a Rust tuple `(T1, T2, ...)`.
fn map_tuple_type(
    api_type: &ApiType,
    known: &KnownTypes<'_>,
    map_inner: fn(&ApiType, &KnownTypes<'_>) -> TokenStream,
) -> TokenStream {
    let values = api_type.tuple_values();
    if values.is_empty() {
        return lua_any_type();
    }
    let types: Vec<_> = values.iter().map(|v| map_inner(v, known)).collect();
    quote!((#(#types),*))
}

/// Maps a `literal` type to its underlying primitive (parameter/return context).
fn map_literal_type(api_type: &ApiType) -> TokenStream {
    match api_type.literal_kind() {
        Some("string") => quote!(&str),
        Some("number") => quote!(f64),
        Some("boolean") => quote!(bool),
        _ => lua_any_type(),
    }
}

/// Maps a `literal` type to its underlying primitive (field/owned context).
fn map_literal_field_type(api_type: &ApiType) -> TokenStream {
    match api_type.literal_kind() {
        Some("string") => quote!(String),
        Some("number") => quote!(f64),
        Some("boolean") => quote!(bool),
        _ => lua_any_type(),
    }
}

fn map_simple_field_type(name: &str, known: &KnownTypes<'_>) -> TokenStream {
    match name {
        // Use owned String for struct fields - &str needs a lifetime parameter.
        "string" | "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => quote!(String),
        "boolean" => quote!(bool),
        "nil" | "void" => quote!(()),
        n if is_integer_api_type(n) || matches!(n, "float" | "double" | "number") => {
            map_numeric_type(n)
        }
        other if known.classes.contains(other) => {
            let ident = make_ident(other);
            // Box<T> breaks potential recursive struct cycles (e.g. LuaForce ↔ LuaTechnology).
            quote!(Box<crate::classes::#ident>)
        }
        other if known.concepts.contains(other) => {
            let ident = make_ident(other);
            quote!(crate::concepts::#ident)
        }
        other if other.starts_with("defines.") => quote!(String),
        _ => lua_any_type(),
    }
}

/// Like [`map_simple_field_type`] but does NOT wrap class references in `Box<T>`.
/// Use this for standalone structs (params structs, inline table structs) that are
/// never embedded inside other structs, so recursive size cycles cannot form.
fn map_simple_field_type_unboxed(name: &str, known: &KnownTypes<'_>) -> TokenStream {
    match name {
        "string" | "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => quote!(String),
        "boolean" => quote!(bool),
        "nil" | "void" => quote!(()),
        n if is_integer_api_type(n) || matches!(n, "float" | "double" | "number") => {
            map_numeric_type(n)
        }
        other if known.classes.contains(other) => {
            let ident = make_ident(other);
            quote!(crate::classes::#ident)
        }
        other if known.concepts.contains(other) => {
            let ident = make_ident(other);
            quote!(crate::concepts::#ident)
        }
        other if other.starts_with("defines.") => quote!(String),
        _ => lua_any_type(),
    }
}

/// Like [`map_simple_field_type`] but produces fully Copy-compatible types:
/// - strings → `&'static str` (Copy; field default is `""`)
/// - class refs → direct `T` (ZSTs after classes become empty structs, Copy)
/// - arrays → `&'static [T]` (fat-pointer, Copy; field default is `&[]`)
/// - concept refs → `crate::LuaAny` (ZST, Copy; avoids potential size cycles)
/// - dicts → `crate::LuaAny` (ZST, Copy)
pub fn map_simple_copy_field_type(name: &str, known: &KnownTypes<'_>) -> TokenStream {
    match name {
        "string" | "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => {
            quote!(&'static str)
        }
        "boolean" => quote!(bool),
        "nil" | "void" => quote!(()),
        n if is_integer_api_type(n) || matches!(n, "float" | "double" | "number") => {
            map_numeric_type(n)
        }
        other if known.classes.contains(other) => {
            let ident = make_ident(other);
            // Classes are now ZSTs - safe to embed directly.
            quote!(crate::classes::#ident)
        }
        other if known.concepts.contains(other) => {
            // Concepts can be mutually recursive (e.g. MapLocation ↔ MapLocation),
            // so we collapse concept-type fields to LuaAny to break size cycles
            // while keeping the outer struct Copy.
            lua_any_type()
        }
        other if other.starts_with("defines.") => quote!(&'static str),
        _ => lua_any_type(),
    }
}

/// Like [`map_field_type`] but produces fully Copy-compatible types.
/// Use for inline table struct fields and concept struct fields where we want
/// everything to be `Copy` so the struct can derive `Copy`.
pub fn map_copy_field_type(api_type: &ApiType, known: &KnownTypes<'_>) -> TokenStream {
    if let Some(name) = api_type.as_simple_name() {
        return map_simple_copy_field_type(name, known);
    }

    match api_type.complex_type() {
        Some("array") => {
            let inner = api_type
                .child_type("value")
                .map(|value| map_copy_field_type(&value, known))
                .unwrap_or_else(lua_any_type);
            quote!(&'static [#inner])
        }
        Some("dictionary") | Some("LuaCustomTable") => lua_any_type(),
        Some("union") => map_union_type(api_type, known, map_copy_field_type),
        Some("type") => api_type
            .child_type("value")
            .map(|value| map_copy_field_type(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("tuple") => map_tuple_type(api_type, known, map_copy_field_type),
        Some("literal") => {
            // Use &'static str (not String) for string literals so the type is Copy.
            match api_type.literal_kind() {
                Some("string") => quote!(&'static str),
                Some("number") => quote!(f64),
                Some("boolean") => quote!(bool),
                _ => lua_any_type(),
            }
        }
        Some("function") | Some("LuaStruct") => lua_any_type(),
        Some("LuaLazyLoadedValue") => api_type
            .child_type("value")
            .map(|value| map_copy_field_type(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("table") => lua_any_type(),
        _ => lua_any_type(),
    }
}

/// Like [`map_field_type`] but does NOT wrap class references in `Box<T>`.
/// Use this for standalone structs (params structs, inline table structs) where
/// recursive size cycles cannot form.
pub fn map_field_type_unboxed(api_type: &ApiType, known: &KnownTypes<'_>) -> TokenStream {
    if let Some(name) = api_type.as_simple_name() {
        return map_simple_field_type_unboxed(name, known);
    }

    match api_type.complex_type() {
        Some("array") => {
            let inner = api_type
                .child_type("value")
                .map(|value| map_field_type_unboxed(&value, known))
                .unwrap_or_else(lua_any_type);
            quote!(Vec<#inner>)
        }
        Some("dictionary") | Some("LuaCustomTable") => {
            map_dict_type(api_type, known, map_field_type_unboxed)
        }
        Some("union") => map_union_type(api_type, known, map_field_type_unboxed),
        Some("type") => api_type
            .child_type("value")
            .map(|value| map_field_type_unboxed(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("tuple") => map_tuple_type(api_type, known, map_field_type_unboxed),
        Some("literal") => map_literal_field_type(api_type),
        Some("function") | Some("LuaStruct") => lua_any_type(),
        Some("LuaLazyLoadedValue") => api_type
            .child_type("value")
            .map(|value| map_field_type_unboxed(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("table") => lua_any_type(),
        _ => lua_any_type(),
    }
}

pub fn map_parameter_stub(
    parameter: &crate::schema::Parameter,
    known: &KnownTypes<'_>,
) -> ReturnStub {
    let mut stub = return_stub_for_type(&parameter.type_name, known);
    if parameter.optional {
        stub = ReturnStub::Option(Box::new(stub));
    }
    stub
}

pub fn map_return_stub(
    return_values: &[crate::schema::Parameter],
    known: &KnownTypes<'_>,
) -> ReturnStub {
    match return_values.len() {
        0 => ReturnStub::Unit,
        1 => map_parameter_stub(&return_values[0], known),
        count => ReturnStub::Tuple(
            return_values
                .iter()
                .take(count)
                .map(|value| map_parameter_stub(value, known))
                .collect(),
        ),
    }
}

pub fn map_parameter_type(
    parameter: &crate::schema::Parameter,
    known: &KnownTypes<'_>,
) -> TokenStream {
    let base = map_api_type(&parameter.type_name, known);
    if parameter.optional {
        quote!(Option<#base>)
    } else {
        base
    }
}

pub fn map_return_type(
    return_values: &[crate::schema::Parameter],
    known: &KnownTypes<'_>,
) -> TokenStream {
    match return_values.len() {
        0 => quote!(()),
        1 => map_parameter_type(&return_values[0], known),
        _ => {
            let types: Vec<_> = return_values
                .iter()
                .map(|value| map_parameter_type(value, known))
                .collect();
            quote!((#(#types),*))
        }
    }
}
