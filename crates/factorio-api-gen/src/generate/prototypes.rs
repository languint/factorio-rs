//! Data-stage prototype stubs from `prototype-api.json`.
//!
//! Every Factorio typename (~260) gets a sparse `Default` stub. Five core
//! prototypes keep hand-curated rich fields; everything else uses automatic
//! field classification against the inheritance chain (common / entity packs,
//! with `LuaAny` escape hatches for required complex properties).

use std::collections::{BTreeMap, HashMap, HashSet};

use proc_macro2::TokenStream;
use quote::quote;

use crate::generate::ident::{make_ident, sanitize_doc};
use crate::schema::ApiType;
use crate::schema_prototype::{PrototypeApi, PrototypeDef, PrototypeProperty};

/// Typenames that use exclusive rich field overrides (not auto-curated).
pub const PROTOTYPE_RICH_OVERRIDES: &[&str] = &[
    "item",
    "recipe",
    "technology",
    "fluid",
    "assembling-machine",
];

const MAX_FIELDS: usize = 40;
const MAX_OPTIONALS: usize = 25;
const ALIAS_DEPTH_LIMIT: usize = 32;

const COMMON_PACK: &[&str] = &[
    "name",
    "order",
    "subgroup",
    "hidden",
    "icons",
    "icon",
    "icon_size",
];

const ENTITY_PACK: &[&str] = &[
    "flags",
    "minable",
    "max_health",
    "collision_box",
    "selection_box",
    "energy_source",
    "energy_usage",
    "crafting_speed",
    "crafting_categories",
    "inventory_size",
    "mining_speed",
    "resource_categories",
    "belt_speed",
    "extension_speed",
    "rotation_speed",
    "module_slots",
    "result_inventory_size",
    "source_inventory_size",
    "fluid_boxes",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Str,
    I64,
    F64,
    Bool,
    OptStr,
    OptI64,
    OptF64,
    OptBool,
    StrSlice,
    OptStrSlice,
    RecipeIngredients,
    RecipeProducts,
    TechPrereqs,
    TechEffects,
    TechUnit,
    Color,
    OptColor,
    BoundingBox,
    EnergySource,
    OptEnergySource,
    Minable,
    LuaAny,
    OptLuaAny,
}

struct FieldSpec {
    name: String,
    kind: FieldKind,
    doc: String,
}

fn rich_fields_for(rust_name: &str) -> Option<Vec<FieldSpec>> {
    let specs: &[(&str, FieldKind, &str)] = match rust_name {
        "Item" => &[
            (
                "name",
                FieldKind::Str,
                "Internal prototype name (e.g. `\"my-mod-widget\"`).",
            ),
            (
                "icon",
                FieldKind::Str,
                "Packaged icon path (e.g. `\"__my_mod__/graphics/icon.png\"`).",
            ),
            (
                "stack_size",
                FieldKind::I64,
                "Max items per inventory slot.",
            ),
            (
                "icon_size",
                FieldKind::OptI64,
                "Icon pixel size. Factorio defaults to `64` when omitted.",
            ),
            (
                "subgroup",
                FieldKind::OptStr,
                "Item subgroup id (e.g. `\"intermediate-product\"`).",
            ),
            (
                "order",
                FieldKind::OptStr,
                "Sort order within the subgroup.",
            ),
        ],
        "Recipe" => &[
            (
                "name",
                FieldKind::Str,
                "Internal prototype name (e.g. `\"my-mod-widget\"`).",
            ),
            (
                "ingredients",
                FieldKind::RecipeIngredients,
                "Crafting ingredients.",
            ),
            ("results", FieldKind::RecipeProducts, "Crafting products."),
            (
                "energy_required",
                FieldKind::OptF64,
                "Crafting energy in seconds. Factorio defaults to `0.5` when omitted.",
            ),
            (
                "category",
                FieldKind::OptStr,
                "Recipe category id (e.g. `\"crafting\"`). Emitted as Lua `category`.",
            ),
            (
                "enabled",
                FieldKind::OptBool,
                "Whether the recipe is unlocked at start.",
            ),
            ("subgroup", FieldKind::OptStr, "Item subgroup id."),
            (
                "order",
                FieldKind::OptStr,
                "Sort order within the subgroup.",
            ),
        ],
        "Technology" => &[
            (
                "name",
                FieldKind::Str,
                "Internal prototype name (e.g. `\"my-mod-widget\"`).",
            ),
            ("icon", FieldKind::Str, "Packaged tech icon path."),
            (
                "icon_size",
                FieldKind::OptI64,
                "Icon pixel size. Technology icons are often `256`.",
            ),
            (
                "prerequisites",
                FieldKind::TechPrereqs,
                "Prerequisite technology ids.",
            ),
            (
                "effects",
                FieldKind::TechEffects,
                "Effects applied on research (typically unlock-recipe).",
            ),
            ("unit", FieldKind::TechUnit, "Lab cost."),
            ("order", FieldKind::OptStr, "Sort order string."),
        ],
        "Fluid" => &[
            ("name", FieldKind::Str, "Internal fluid prototype name."),
            ("icon", FieldKind::Str, "Packaged icon path."),
            (
                "default_temperature",
                FieldKind::F64,
                "Default temperature of the fluid.",
            ),
            ("base_color", FieldKind::Color, "Primary fluid color."),
            ("flow_color", FieldKind::Color, "Flow / animation color."),
            ("icon_size", FieldKind::OptI64, "Icon pixel size."),
            ("subgroup", FieldKind::OptStr, "Item subgroup id."),
            ("order", FieldKind::OptStr, "Sort order string."),
            (
                "hidden",
                FieldKind::OptBool,
                "Hide from factoriopedia / lists when true.",
            ),
        ],
        "AssemblingMachine" => &[
            ("name", FieldKind::Str, "Internal entity prototype name."),
            ("icon", FieldKind::Str, "Packaged icon path."),
            (
                "crafting_speed",
                FieldKind::F64,
                "Crafting speed multiplier.",
            ),
            (
                "crafting_categories",
                FieldKind::StrSlice,
                "Recipe category ids this machine accepts.",
            ),
            (
                "energy_usage",
                FieldKind::Str,
                "Energy usage string (e.g. `\"150kW\"`).",
            ),
            (
                "energy_source",
                FieldKind::EnergySource,
                "Simplified energy source table.",
            ),
            ("icon_size", FieldKind::OptI64, "Icon pixel size."),
            (
                "flags",
                FieldKind::OptStrSlice,
                "Entity flags (e.g. `placeable-neutral`, `player-creation`).",
            ),
            (
                "minable",
                FieldKind::Minable,
                "Mining properties when the entity is mined.",
            ),
            ("max_health", FieldKind::OptF64, "Maximum health."),
            ("collision_box", FieldKind::BoundingBox, "Collision box."),
            ("selection_box", FieldKind::BoundingBox, "Selection box."),
            ("module_slots", FieldKind::OptI64, "Number of module slots."),
            ("subgroup", FieldKind::OptStr, "Item subgroup id."),
            ("order", FieldKind::OptStr, "Sort order string."),
        ],
        _ => return None,
    };
    Some(
        specs
            .iter()
            .map(|(name, kind, doc)| FieldSpec {
                name: (*name).to_string(),
                kind: *kind,
                doc: (*doc).to_string(),
            })
            .collect(),
    )
}

/// JSON property name used for validation (Recipe `category` is curated-only).
fn json_property_name<'a>(rust_struct: &str, field: &'a str) -> Option<&'a str> {
    match (rust_struct, field) {
        ("Recipe", "category") => None, // curated mid-mod alias; JSON uses `categories`
        _ => Some(field),
    }
}

fn rust_type_tokens(kind: FieldKind) -> TokenStream {
    match kind {
        FieldKind::Str => quote! { &'static str },
        FieldKind::I64 => quote! { i64 },
        FieldKind::F64 => quote! { f64 },
        FieldKind::Bool => quote! { bool },
        FieldKind::OptStr => quote! { Option<&'static str> },
        FieldKind::OptI64 => quote! { Option<i64> },
        FieldKind::OptF64 => quote! { Option<f64> },
        FieldKind::OptBool => quote! { Option<bool> },
        FieldKind::StrSlice => quote! { &'static [&'static str] },
        FieldKind::OptStrSlice => quote! { Option<&'static [&'static str]> },
        FieldKind::RecipeIngredients => quote! { &'static [crate::prototypes::RecipeIngredient] },
        FieldKind::RecipeProducts => quote! { &'static [crate::prototypes::RecipeProduct] },
        FieldKind::TechPrereqs => quote! { &'static [&'static str] },
        FieldKind::TechEffects => quote! { &'static [crate::prototypes::UnlockRecipeEffect] },
        FieldKind::TechUnit => quote! { crate::prototypes::TechnologyUnit },
        FieldKind::Color => quote! { crate::prototypes::Color },
        FieldKind::OptColor => quote! { Option<crate::prototypes::Color> },
        FieldKind::BoundingBox => quote! { Option<crate::prototypes::BoundingBox> },
        FieldKind::EnergySource => quote! { crate::prototypes::EnergySource },
        FieldKind::OptEnergySource => quote! { Option<crate::prototypes::EnergySource> },
        FieldKind::Minable => quote! { Option<crate::prototypes::MinableProperties> },
        FieldKind::LuaAny => quote! { crate::LuaAny },
        FieldKind::OptLuaAny => quote! { Option<crate::LuaAny> },
    }
}

fn needs_eq(kind: FieldKind) -> bool {
    !matches!(
        kind,
        FieldKind::F64
            | FieldKind::OptF64
            | FieldKind::TechUnit
            | FieldKind::Color
            | FieldKind::OptColor
            | FieldKind::BoundingBox
            | FieldKind::EnergySource
            | FieldKind::OptEnergySource
            | FieldKind::Minable
            | FieldKind::RecipeIngredients
            | FieldKind::RecipeProducts
            | FieldKind::TechEffects
    )
}

fn rust_struct_name(proto: &PrototypeDef) -> String {
    if let Some(stripped) = proto.name.strip_suffix("Prototype")
        && !stripped.is_empty()
    {
        return stripped.to_string();
    }
    let Some(typename) = proto.typename.as_deref() else {
        return proto.name.clone();
    };
    typename
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

fn build_alias_map(api: &PrototypeApi) -> HashMap<&str, &ApiType> {
    api.types
        .iter()
        .map(|t| (t.name.as_str(), &t.type_name))
        .collect()
}

fn is_skip_type_name(name: &str) -> bool {
    matches!(
        name,
        "Sprite"
            | "Sound"
            | "IconData"
            | "FluidBox"
            | "Vector"
            | "TriggerEffect"
            | "AttackParameters"
            | "SimulationDefinition"
            | "MouseCursorID"
            | "Animation4Way"
            | "Sprite4Way"
            | "RotatedSprite"
            | "RotatedAnimation"
            | "Animation"
            | "Trigger"
            | "TriggerItem"
            | "DirectTriggerItem"
            | "AreaTriggerItem"
            | "LineTriggerItem"
            | "ClusterTriggerItem"
    ) || name.starts_with("Animation")
        || name.starts_with("Sprite")
        || name.starts_with("Sound")
        || name.starts_with("Trigger")
        || name.starts_with("AttackParameters")
        || name.starts_with("Autoplace")
        || name.starts_with("graphics")
        || name.starts_with("Graphics")
        || name.starts_with("CollisionMask")
        || name.starts_with("CircuitConnector")
        || name.contains("Animation")
            && (name.contains("Specification") || name.ends_with("Variations"))
}

fn is_integer_name(name: &str) -> bool {
    matches!(
        name,
        "ItemCountType" | "ItemStackIndex" | "SpriteSizeType" | "MapTick" | "Tick"
    ) || name.starts_with("uint")
        || name.starts_with("int")
}

fn is_float_name(name: &str) -> bool {
    matches!(name, "float" | "double" | "number")
}

fn is_stringish_name(name: &str) -> bool {
    matches!(
        name,
        "string" | "FileName" | "Order" | "Energy" | "LocalisedString" | "RenderLayer"
    ) || name.ends_with("ID")
}

fn opt(kind_req: FieldKind, kind_opt: FieldKind, optional: bool) -> FieldKind {
    if optional { kind_opt } else { kind_req }
}

fn classify_array_elem(
    elem: &ApiType,
    aliases: &HashMap<&str, &ApiType>,
    optional: bool,
    depth: usize,
) -> Option<FieldKind> {
    if let Some(name) = elem.as_simple_name() {
        if is_stringish_name(name) {
            return Some(opt(FieldKind::StrSlice, FieldKind::OptStrSlice, optional));
        }
        if is_skip_type_name(name) {
            return None;
        }
        if let Some(aliased) = aliases.get(name) {
            // e.g. flags alias, or ID typedef -> string
            return match classify_type(aliased, aliases, false, depth + 1)? {
                FieldKind::Str | FieldKind::OptStr => {
                    Some(opt(FieldKind::StrSlice, FieldKind::OptStrSlice, optional))
                }
                FieldKind::StrSlice | FieldKind::OptStrSlice => {
                    Some(opt(FieldKind::StrSlice, FieldKind::OptStrSlice, optional))
                }
                _ => None,
            };
        }
        return None;
    }
    if elem.is_homog_string_literal_union() {
        return Some(opt(FieldKind::StrSlice, FieldKind::OptStrSlice, optional));
    }
    None
}

fn classify_simple_name(name: &str, optional: bool) -> Option<FieldKind> {
    if is_skip_type_name(name) {
        return None;
    }
    if name == "boolean" {
        return Some(opt(FieldKind::Bool, FieldKind::OptBool, optional));
    }
    if is_stringish_name(name) {
        return Some(opt(FieldKind::Str, FieldKind::OptStr, optional));
    }
    if is_float_name(name) {
        return Some(opt(FieldKind::F64, FieldKind::OptF64, optional));
    }
    if is_integer_name(name) {
        return Some(opt(FieldKind::I64, FieldKind::OptI64, optional));
    }
    if name == "Color" {
        return Some(opt(FieldKind::Color, FieldKind::OptColor, optional));
    }
    if name == "BoundingBox" {
        return Some(FieldKind::BoundingBox);
    }
    if name == "MinableProperties" {
        return Some(FieldKind::Minable);
    }
    if matches!(
        name,
        "EnergySource" | "ElectricEnergySource" | "BurnerEnergySource"
    ) {
        return Some(opt(
            FieldKind::EnergySource,
            FieldKind::OptEnergySource,
            optional,
        ));
    }
    None
}

fn classify_type(
    ty: &ApiType,
    aliases: &HashMap<&str, &ApiType>,
    optional: bool,
    depth: usize,
) -> Option<FieldKind> {
    if depth >= ALIAS_DEPTH_LIMIT {
        return None;
    }

    if let Some(name) = ty.as_simple_name() {
        if let Some(kind) = classify_simple_name(name, optional) {
            return Some(kind);
        }
        if is_skip_type_name(name) {
            return None;
        }
        if let Some(aliased) = aliases.get(name) {
            return classify_type(aliased, aliases, optional, depth + 1);
        }
        return None;
    }

    match ty.complex_type() {
        Some("array") => {
            let elem = ty.child_type("value")?;
            classify_array_elem(&elem, aliases, optional, depth + 1)
        }
        Some("union") => {
            if ty.is_homog_string_literal_union() {
                return Some(opt(FieldKind::Str, FieldKind::OptStr, optional));
            }
            // Skip heterogeneous unions, dictionaries wrapped as unions, etc.
            None
        }
        Some("type") => {
            let value = ty.0.get("value")?.as_str()?;
            classify_type(
                &ApiType(serde_json::Value::String(value.to_string())),
                aliases,
                optional,
                depth + 1,
            )
        }
        Some("literal") => match ty.literal_kind() {
            Some("string") => Some(opt(FieldKind::Str, FieldKind::OptStr, optional)),
            Some("number") => Some(opt(FieldKind::F64, FieldKind::OptF64, optional)),
            Some("boolean") => Some(opt(FieldKind::Bool, FieldKind::OptBool, optional)),
            _ => None,
        },
        // dictionary, struct, tuple, table, ...
        Some("dictionary" | "struct" | "tuple" | "table" | "LuaStruct") => None,
        _ => None,
    }
}

fn classify_property(
    prop: &PrototypeProperty,
    aliases: &HashMap<&str, &ApiType>,
) -> Option<FieldKind> {
    classify_type(&prop.type_name, aliases, prop.optional, 0)
}

fn is_entity_prototype(proto: &PrototypeDef, by_name: &HashMap<&str, &PrototypeDef>) -> bool {
    let mut current = Some(proto);
    while let Some(p) = current {
        if p.name.contains("Entity") {
            return true;
        }
        current = p
            .parent
            .as_deref()
            .and_then(|parent| by_name.get(parent).copied());
    }
    false
}

fn in_field_pack(name: &str, entity: bool) -> bool {
    if COMMON_PACK.contains(&name) {
        return true;
    }
    if entity {
        if ENTITY_PACK.contains(&name) {
            return true;
        }
        if name.starts_with("inserter_") {
            return true;
        }
    }
    false
}

fn field_doc(prop: &PrototypeProperty) -> String {
    let desc = sanitize_doc(&prop.description);
    if desc.is_empty() {
        format!("Prototype property `{}`.", prop.name)
    } else {
        desc
    }
}

fn auto_fields_for(
    proto: &PrototypeDef,
    by_name: &HashMap<&str, &PrototypeDef>,
    aliases: &HashMap<&str, &ApiType>,
) -> Vec<FieldSpec> {
    let inherited = collect_properties(proto, by_name);
    let entity = is_entity_prototype(proto, by_name);

    let mut required = Vec::new();
    let mut pack_optionals = Vec::new();
    let mut other_optionals = Vec::new();

    for (name, prop) in &inherited {
        // `type` is injected by the Lua generator.
        if name == "type" {
            continue;
        }

        let pack = in_field_pack(name, entity);
        let classified = classify_property(prop, aliases);

        if prop.optional {
            let kind = match classified {
                Some(k) => k,
                None if pack && name == "fluid_boxes" => FieldKind::OptLuaAny,
                None => continue, // skip complex optionals
            };
            // Prefer skipping OptLuaAny outside packs.
            if matches!(kind, FieldKind::OptLuaAny | FieldKind::LuaAny) && !pack {
                continue;
            }
            // icons: only if classifiable (IconData arrays are skipped).
            let spec = FieldSpec {
                name: name.clone(),
                kind,
                doc: field_doc(prop),
            };
            if pack {
                pack_optionals.push(spec);
            } else {
                other_optionals.push(spec);
            }
        } else {
            let kind = classified.unwrap_or(FieldKind::LuaAny);
            required.push(FieldSpec {
                name: name.clone(),
                kind,
                doc: field_doc(prop),
            });
        }
    }

    // Prefer concrete required fields over LuaAny when capping.
    required.sort_by_key(|f| matches!(f.kind, FieldKind::LuaAny));

    let mut out = Vec::new();
    for field in required {
        if out.len() >= MAX_FIELDS {
            break;
        }
        out.push(field);
    }

    for (opt_count, field) in pack_optionals
        .into_iter()
        .chain(other_optionals)
        .enumerate()
    {
        if out.len() >= MAX_FIELDS || opt_count >= MAX_OPTIONALS {
            break;
        }
        out.push(field);
    }

    out
}

/// Generate sparse prototype stubs for every typename in the API.
pub fn generate_prototypes(api: &PrototypeApi) -> Result<String, String> {
    let by_name: HashMap<&str, &PrototypeDef> = api
        .prototypes
        .iter()
        .map(|p| (p.name.as_str(), p))
        .collect();
    let aliases = build_alias_map(api);

    let mut typename_index: BTreeMap<&str, &PrototypeDef> = BTreeMap::new();
    for proto in &api.prototypes {
        if let Some(typename) = proto.typename.as_deref() {
            typename_index.insert(typename, proto);
        }
    }

    let rich: HashSet<&str> = PROTOTYPE_RICH_OVERRIDES.iter().copied().collect();

    let mut entries: Vec<(&str, String, &PrototypeDef, Vec<FieldSpec>)> = Vec::new();
    for (typename, proto) in &typename_index {
        let rust_name = rust_struct_name(proto);
        let fields = if rich.contains(typename) {
            let Some(fields) = rich_fields_for(&rust_name) else {
                return Err(format!(
                    "rich override missing fields for `{rust_name}` ({typename})"
                ));
            };
            let inherited = collect_properties(proto, &by_name);
            let prop_names: HashSet<&str> = inherited.iter().map(|(n, _)| n.as_str()).collect();
            for field in &fields {
                if let Some(json_name) = json_property_name(&rust_name, &field.name)
                    && !prop_names.contains(json_name)
                {
                    return Err(format!(
                        "rich override field `{}.{}` not found on {} parent chain",
                        rust_name, field.name, proto.name
                    ));
                }
            }
            fields
        } else {
            auto_fields_for(proto, &by_name, &aliases)
        };
        entries.push((typename, rust_name, proto, fields));
    }

    entries.sort_by(|a, b| a.1.cmp(&b.1));

    let mut structs = Vec::new();
    for (typename, rust_name, proto, fields) in &entries {
        structs.push(emit_struct(rust_name, typename, proto, fields));
    }

    let header = format!(
        "// Generated from Factorio prototype API v{} (format v{}).\n\
         // Sparse stubs for every typename. Companions live in prototypes.rs.\n\
         #[allow(unused, clippy::all, clippy::pedantic, clippy::nursery)]\n\n",
        api.application_version, api.api_version
    );

    Ok(format!(
        "{header}{}",
        structs.into_iter().collect::<TokenStream>()
    ))
}

/// Emit `prototype_lua_typename` mapping Rust struct names -> Factorio typenames.
pub fn generate_prototype_type_map(api: &PrototypeApi) -> Result<String, String> {
    let mut arms: BTreeMap<String, String> = BTreeMap::new();

    for proto in &api.prototypes {
        let Some(typename) = proto.typename.as_deref() else {
            continue;
        };
        let rust_name = rust_struct_name(proto);
        arms.insert(rust_name, typename.to_string());
    }

    // Companion / settings special cases (not typenames in prototype-api).
    for (rust_name, typename) in [
        ("RecipeProduct", "item"),
        ("UnlockRecipeEffect", "unlock-recipe"),
        ("BoolSetting", "bool-setting"),
        ("IntSetting", "int-setting"),
        ("DoubleSetting", "double-setting"),
        ("StringSetting", "string-setting"),
    ] {
        arms.entry(rust_name.to_string())
            .or_insert_with(|| typename.to_string());
    }

    let match_arms: Vec<_> = arms
        .iter()
        .map(|(rust_name, typename)| {
            let ident = rust_name.as_str();
            quote! { #ident => Some(#typename), }
        })
        .collect();

    let tokens = quote! {
        /// Map a Rust prototype / companion struct name to its Factorio `type` string.
        pub fn prototype_lua_typename(struct_name: &str) -> Option<&'static str> {
            match struct_name {
                #(#match_arms)*
                _ => None,
            }
        }
    };

    let header = format!(
        "// Generated from Factorio prototype API v{} (format v{}).\n\
         #[allow(unused, clippy::all, clippy::pedantic, clippy::nursery)]\n\n",
        api.application_version, api.api_version
    );

    Ok(format!("{header}{tokens}"))
}

fn collect_properties<'a>(
    proto: &'a PrototypeDef,
    by_name: &HashMap<&'a str, &'a PrototypeDef>,
) -> Vec<(String, &'a PrototypeProperty)> {
    let mut chain = Vec::new();
    let mut current = Some(proto);
    while let Some(p) = current {
        chain.push(p);
        current = p
            .parent
            .as_deref()
            .and_then(|parent| by_name.get(parent).copied());
    }
    chain.reverse();

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for ancestor in chain {
        for prop in &ancestor.properties {
            if seen.insert(prop.name.as_str()) {
                out.push((prop.name.clone(), prop));
            }
        }
    }
    out
}

fn emit_struct(
    rust_name: &str,
    typename: &str,
    proto: &PrototypeDef,
    fields: &[FieldSpec],
) -> TokenStream {
    let ident = make_ident(rust_name);
    let link = format!(
        "Minimal [`{}`](https://lua-api.factorio.com/latest/prototypes/{}.html) for `data.extend`.",
        proto.name, proto.name
    );
    let type_doc = format!("`type = \"{typename}\"` is injected by the Lua generator.");
    let desc = sanitize_doc(&proto.description);
    let doc = if desc.is_empty() {
        format!("{link}\n\n{type_doc}")
    } else {
        format!("{link}\n\n{desc}\n\n{type_doc}")
    };

    let mut all_eq = true;
    let field_tokens: Vec<_> = fields
        .iter()
        .map(|field| {
            if !needs_eq(field.kind) {
                all_eq = false;
            }
            let name = make_ident(&field.name);
            let ty = rust_type_tokens(field.kind);
            let field_doc = field.doc.as_str();
            quote! {
                #[doc = #field_doc]
                pub #name: #ty,
            }
        })
        .collect();

    let derives = if all_eq {
        quote! { #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] }
    } else {
        quote! { #[derive(Debug, Clone, Copy, PartialEq, Default)] }
    };

    quote! {
        #[doc = #doc]
        #derives
        pub struct #ident {
            #(#field_tokens)*
        }
    }
}
