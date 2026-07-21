use std::collections::{BTreeSet, HashMap};

use crate::generate::ident::make_ident;
use crate::generate::unions::UnionRegistry;
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
    /// Identification / mixed-union enums emitted into `crate::concepts`.
    pub identifications: &'a BTreeSet<String>,
    /// Sorted arm signature keys -> identification concept name.
    pub identification_signatures: &'a HashMap<Vec<String>, String>,
    /// `crate::unions::*` - Copy unit enums for homog string-literal unions.
    pub unions: &'a BTreeSet<String>,
    /// Registry used to resolve anonymous/named literal unions to enum names.
    pub union_registry: &'a UnionRegistry,
    /// Flag-set concepts (`MouseButtonFlags`, ...) lowered as dict-of-true tables.
    pub flag_sets: &'a BTreeSet<String>,
}

fn union_type_path(name: &str) -> TokenStream {
    let ident = make_ident(name);
    quote!(crate::unions::#ident)
}

/// Opaque placeholder for complex Factorio Lua API values.
pub fn lua_any_type() -> TokenStream {
    quote!(crate::LuaAny)
}

fn lua_function_type() -> TokenStream {
    quote!(crate::LuaFunction)
}

/// Whether this API type is (or unwraps to) a Factorio `function` complex type.
fn is_function_api_type(api_type: &ApiType) -> bool {
    let ty = if api_type.complex_type() == Some("type") {
        api_type
            .child_type("value")
            .unwrap_or_else(|| api_type.clone())
    } else {
        api_type.clone()
    };
    ty.complex_type() == Some("function")
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
            "string" => ReturnStub::Str,
            "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => ReturnStub::Default,
            "EventFilter" => ReturnStub::Vec(Box::new(ReturnStub::Default)),
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
            other if known.unions.contains(other) => ReturnStub::Default,
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
                    if let Some(enum_name) = known.union_registry.resolve(api_type) {
                        let _ = enum_name;
                        if has_nil {
                            ReturnStub::Option(Box::new(ReturnStub::Default))
                        } else {
                            ReturnStub::Default
                        }
                    } else if all_same_literal_kind(&non_nil) {
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
                    } else if map_class_or_string_union(&non_nil, known, ClassOrStringPrefer::Class)
                        .is_some()
                    {
                        let stub = ReturnStub::Default;
                        if has_nil {
                            ReturnStub::Option(Box::new(stub))
                        } else {
                            stub
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
        Some("function") => ReturnStub::Default,
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
        Some("function") => lua_function_type(),
        Some("LuaStruct") => lua_any_type(),
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
/// Integer types preserve their exact width (e.g. `uint16` -> `u16`) so that
/// callers get useful range information and IDE diagnostics. Float types use the
/// appropriate float (`float` -> `f32`, `double`/`number` -> `f64`).
///
/// All of these are `Copy`, so the mapping is safe for every context.
pub fn map_numeric_type_tokens(name: &str) -> TokenStream {
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

fn map_numeric_type(name: &str) -> TokenStream {
    map_numeric_type_tokens(name)
}

/// Returns `true` for all Factorio numeric type names that should be treated as integers.
pub fn is_integer_api_type_pub(name: &str) -> bool {
    is_integer_api_type(name)
}

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
        "string" => quote!(&'static str),
        "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => {
            quote!(crate::LocalisedString)
        }
        "EventFilter" => quote!(Vec<crate::EventFilterEntry>),
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
        other if known.unions.contains(other) => union_type_path(other),
        other if other.starts_with("defines.") => quote!(&'static str),
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
        Some("function") => lua_function_type(),
        Some("LuaStruct") => lua_any_type(),
        Some("LuaLazyLoadedValue") => api_type
            .child_type("value")
            .map(|value| map_field_type(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("table") => lua_any_type(),
        _ => lua_any_type(),
    }
}

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
/// - Homogeneous string-literal unions -> generated unit enum (`crate::unions::*`)
/// - Single non-nil arm -> that arm's type
/// - Single non-nil arm + nil(s) -> `Option<T>`
/// - Multiple non-nil arms of different/complex types -> `LuaAny`
fn map_union_type(
    api_type: &ApiType,
    known: &KnownTypes<'_>,
    map_inner: fn(&ApiType, &KnownTypes<'_>) -> TokenStream,
) -> TokenStream {
    if let Some(enum_name) = known.union_registry.resolve(api_type) {
        let ty = union_type_path(enum_name);
        return if api_type.union_has_nil() {
            quote!(Option<#ty>)
        } else {
            ty
        };
    }

    let options = api_type.options();
    let non_nil: Vec<_> = options
        .iter()
        .filter(|o| o.as_simple_name() != Some("nil"))
        .collect();
    let has_nil = options.len() > non_nil.len();

    if let Some(name) = resolve_identification_for_arms(&non_nil, known) {
        let ident = make_ident(&name);
        let ty = quote!(crate::concepts::#ident);
        return if has_nil { quote!(Option<#ty>) } else { ty };
    }

    if let Some(elem) = scalar_from_scalar_or_array(&non_nil) {
        let ty = map_inner(elem, known);
        return if has_nil { quote!(Option<#ty>) } else { ty };
    }

    // Anonymous `uint32 | string` (get_player, get_surface, ...).
    if let Some(ty) = map_index_or_name_union(&non_nil) {
        return if has_nil { quote!(Option<#ty>) } else { ty };
    }

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
            // Non-string homogeneous literals (numbers/bools) still collapse to primitives.
            if all_same_literal_kind(&non_nil) {
                let ty = map_inner(non_nil[0], known);
                if has_nil { quote!(Option<#ty>) } else { ty }
            } else if let Some(ty) =
                map_class_or_string_union(&non_nil, known, ClassOrStringPrefer::Class)
            {
                if has_nil { quote!(Option<#ty>) } else { ty }
            } else {
                lua_any_type()
            }
        }
    }
}

/// `LuaStyle | string` (and similar): prefer the class for reads / the string for writes.
#[derive(Clone, Copy)]
pub(crate) enum ClassOrStringPrefer {
    Class,
    String,
}

pub(crate) fn map_class_or_string_union(
    arms: &[&ApiType],
    known: &KnownTypes<'_>,
    prefer: ClassOrStringPrefer,
) -> Option<TokenStream> {
    if arms.len() != 2 {
        return None;
    }
    let mut class_name: Option<String> = None;
    let mut has_string = false;
    for arm in arms {
        let arm = unwrap_type_ref(arm);
        match arm.as_simple_name() {
            Some("string") => has_string = true,
            Some(name) if known.classes.contains(name) => {
                if class_name.is_some() {
                    return None;
                }
                class_name = Some(name.to_string());
            }
            _ => return None,
        }
    }
    let class_name = class_name?;
    if !has_string {
        return None;
    }
    Some(match prefer {
        ClassOrStringPrefer::Class => {
            let ident = make_ident(&class_name);
            quote!(crate::classes::#ident)
        }
        ClassOrStringPrefer::String => quote!(&'static str),
    })
}

fn map_index_or_name_union(arms: &[&ApiType]) -> Option<TokenStream> {
    if arms.len() != 2 {
        return None;
    }
    let mut names: Vec<String> = arms
        .iter()
        .filter_map(|arm| {
            let arm = unwrap_type_ref(arm);
            arm.as_simple_name().map(str::to_string)
        })
        .collect();
    if names.len() != 2 {
        return None;
    }
    names.sort_unstable();
    if names == ["string", "uint32"] {
        return Some(quote!(crate::IndexOrName));
    }
    None
}

/// If arms are exactly `T` and `array<T>` (either order), return `T`.
fn scalar_from_scalar_or_array<'a>(arms: &[&'a ApiType]) -> Option<&'a ApiType> {
    if arms.len() != 2 {
        return None;
    }
    let (a, b) = (arms[0], arms[1]);
    if a.complex_type() == Some("array")
        && let Some(value) = a.child_type("value")
        && types_equivalent(&value, b)
    {
        return Some(b);
    }
    if b.complex_type() == Some("array")
        && let Some(value) = b.child_type("value")
        && types_equivalent(&value, a)
    {
        return Some(a);
    }
    None
}

fn types_equivalent(a: &ApiType, b: &ApiType) -> bool {
    match (a.as_simple_name(), b.as_simple_name()) {
        (Some(x), Some(y)) => x == y,
        _ => a.0 == b.0,
    }
}

fn unwrap_type_ref(api_type: &ApiType) -> ApiType {
    if api_type.complex_type() == Some("type")
        && let Some(inner) = api_type.child_type("value")
    {
        return unwrap_type_ref(&inner);
    }
    api_type.clone()
}

fn arm_signature_key(arm: &ApiType) -> Option<String> {
    let arm = unwrap_type_ref(arm);
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

fn resolve_identification_for_arms(arms: &[&ApiType], known: &KnownTypes<'_>) -> Option<String> {
    let mut keys: Vec<String> = arms
        .iter()
        .filter_map(|arm| arm_signature_key(arm))
        .collect();
    if keys.len() != arms.len() {
        return None;
    }
    keys.sort();
    known.identification_signatures.get(&keys).cloned()
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
        Some("string") => quote!(&'static str),
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
        "EventFilter" => quote!(Vec<crate::EventFilterEntry>),
        "boolean" => quote!(bool),
        "nil" | "void" => quote!(()),
        n if is_integer_api_type(n) || matches!(n, "float" | "double" | "number") => {
            map_numeric_type(n)
        }
        other if known.classes.contains(other) => {
            let ident = make_ident(other);
            // Box<T> breaks srecursive struct cycles.
            quote!(Box<crate::classes::#ident>)
        }
        other if known.concepts.contains(other) => {
            let ident = make_ident(other);
            quote!(crate::concepts::#ident)
        }
        other if known.unions.contains(other) => union_type_path(other),
        other if other.starts_with("defines.") => quote!(String),
        _ => lua_any_type(),
    }
}

fn map_simple_field_type_unboxed(name: &str, known: &KnownTypes<'_>) -> TokenStream {
    match name {
        "string" | "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => quote!(String),
        "EventFilter" => quote!(Vec<crate::EventFilterEntry>),
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
        other if known.unions.contains(other) => union_type_path(other),
        other if other.starts_with("defines.") => quote!(String),
        _ => lua_any_type(),
    }
}

pub fn map_simple_copy_field_type(name: &str, known: &KnownTypes<'_>) -> TokenStream {
    match name {
        "string" => quote!(&'static str),
        "LocalisedString" | "LuaLazyLoadedValueLocalisedString" => {
            quote!(crate::LocalisedString)
        }
        "EventFilter" => quote!(&'static [crate::EventFilterEntry]),
        "boolean" => quote!(bool),
        "nil" | "void" => quote!(()),
        n if is_integer_api_type(n) || matches!(n, "float" | "double" | "number") => {
            map_numeric_type(n)
        }
        other if known.classes.contains(other) => {
            let ident = make_ident(other);
            quote!(crate::classes::#ident)
        }
        other if known.unions.contains(other) => union_type_path(other),
        other if known.concepts.contains(other) => {
            let ident = make_ident(other);
            quote!(crate::concepts::#ident)
        }
        other if other.starts_with("defines.") => quote!(&'static str),
        _ => lua_any_type(),
    }
}

/// Like [`map_copy_field_type`], but leaves a self-referential concept field as
/// `LuaAny` so the parent struct can stay `Copy` (e.g. `MapLocation.position`).
pub fn map_copy_field_type_for_concept(
    api_type: &ApiType,
    known: &KnownTypes<'_>,
    parent_concept: &str,
) -> TokenStream {
    if let Some(name) = api_type.as_simple_name()
        && name == parent_concept
        && known.concepts.contains(name)
    {
        return lua_any_type();
    }
    map_copy_field_type(api_type, known)
}

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
        Some("function") => lua_function_type(),
        Some("LuaStruct") => lua_any_type(),
        Some("LuaLazyLoadedValue") => api_type
            .child_type("value")
            .map(|value| map_copy_field_type(&value, known))
            .unwrap_or_else(lua_any_type),
        Some("table") => lua_any_type(),
        _ => lua_any_type(),
    }
}

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
        Some("function") => lua_function_type(),
        Some("LuaStruct") => lua_any_type(),
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
    // Callback parameters accept Rust `fn` items via `Into<LuaFunction>`.
    if let Some(tokens) = map_function_parameter_type(parameter) {
        return tokens;
    }
    if let Some(tokens) = map_localised_string_parameter(parameter) {
        return tokens;
    }
    let base = map_api_type(&parameter.type_name, known);
    if parameter.optional {
        quote!(Option<#base>)
    } else {
        base
    }
}

fn map_localised_string_parameter(parameter: &crate::schema::Parameter) -> Option<TokenStream> {
    let ty = &parameter.type_name;
    let is_localised = matches!(
        ty.as_simple_name(),
        Some("LocalisedString" | "LuaLazyLoadedValueLocalisedString")
    );
    if !is_localised {
        return None;
    }
    Some(if parameter.optional {
        quote!(Option<crate::LocalisedString>)
    } else {
        quote!(impl Into<crate::LocalisedString>)
    })
}

fn map_function_parameter_type(parameter: &crate::schema::Parameter) -> Option<TokenStream> {
    let ty = &parameter.type_name;
    if is_function_api_type(ty) {
        return Some(if parameter.optional {
            // Rare: optional function without a nil union arm.
            quote!(impl crate::IntoOptionalLuaFunction)
        } else {
            quote!(impl Into<crate::LuaFunction>)
        });
    }
    if ty.complex_type() == Some("union") {
        let non_nil = ty.non_nil_options();
        if non_nil.len() == 1 && is_function_api_type(&non_nil[0]) && ty.union_has_nil() {
            // `function | nil` - pass a handler or `None` to unregister.
            return Some(quote!(impl crate::IntoOptionalLuaFunction));
        }
    }
    None
}

pub fn map_return_type(
    return_values: &[crate::schema::Parameter],
    known: &KnownTypes<'_>,
) -> TokenStream {
    match return_values.len() {
        0 => quote!(()),
        1 => map_return_value_type(&return_values[0], known),
        _ => {
            let types: Vec<_> = return_values
                .iter()
                .map(|value| map_return_value_type(value, known))
                .collect();
            quote!((#(#types),*))
        }
    }
}

fn map_return_value_type(
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
