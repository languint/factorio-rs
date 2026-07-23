//! Frontend expansion for additional data-stage prototype macros.

use std::fmt::Write;

use proc_macro2::TokenStream;
use syn::{
    Ident, LitBool, LitFloat, LitInt, LitStr, Token,
    parse::{Parse, ParseStream},
};

use super::proto_macros::{
    color_src, emit_names_module, energy_source_src, option_bool_src, option_f64_src,
    option_flags_src, option_i64_src, option_str_src, resolve_optional_icon, str_list_src,
};
use crate::error::{FrontendError, FrontendResult};

macro_rules! expand_proto {
    ($name:ident, $input:ty, $entries:ident, $names:literal, $register:literal, $build:expr) => {
        pub fn $name(
            tokens: TokenStream,
            mod_name: Option<&str>,
        ) -> FrontendResult<Vec<syn::Item>> {
            let input: $input =
                syn::parse2(tokens).map_err(FrontendError::from)?;
            let mut const_defs = String::new();
            let mut extend_items = String::new();
            for entry in &input.$entries {
                let const_name = entry.ident.to_string().to_uppercase();
                let _ = writeln!(
                    const_defs,
                    "pub const {const_name}: &'static str = \"{}\";",
                    entry.name
                );
                let item = ($build)(entry, mod_name)?;
                let _ = writeln!(extend_items, "{item},");
            }
            emit_names_module($names, $register, &const_defs, &extend_items)
        }
    };
}

expand_proto!(
    expand_container,
    ContainersInput,
    entries,
    "Containers",
    "register_containers",
    |e: &ContainerEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        // icon_size accepted in DSL but not present on sparse Container stub.
        let _ = e.icon_size;
        Ok(format!(
            "Container {{ name: \"{}\", inventory_size: {}, icon: {icon}, subgroup: {}, order: {}, flags: {}, max_health: {}, ..Default::default() }}",
            e.name,
            e.inventory_size,
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
            option_flags_src(e.flags.as_deref()),
            option_f64_src(e.max_health),
        ))
    }
);

expand_proto!(
    expand_inserter,
    InsertersInput,
    entries,
    "Inserters",
    "register_inserters",
    |e: &InserterEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        let _ = &e.energy_usage; // DSL default; sparse Inserter has no energy_usage field
        Ok(format!(
            "Inserter {{ name: \"{}\", extension_speed: {}, rotation_speed: {}, energy_source: {}, icon: {icon}, subgroup: {}, order: {}, flags: {}, max_health: {}, ..Default::default() }}",
            e.name,
            e.extension_speed,
            e.rotation_speed,
            energy_source_src(&e.energy_type, e.usage_priority.as_deref()),
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
            option_flags_src(e.flags.as_deref()),
            option_f64_src(e.max_health),
        ))
    }
);

expand_proto!(
    expand_transport_belt,
    TransportBeltsInput,
    entries,
    "TransportBelts",
    "register_transport_belts",
    |e: &TransportBeltEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        Ok(format!(
            "TransportBelt {{ name: \"{}\", speed: {}, icon: {icon}, subgroup: {}, order: {}, flags: {}, max_health: {}, ..Default::default() }}",
            e.name,
            e.speed,
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
            option_flags_src(e.flags.as_deref()),
            option_f64_src(e.max_health),
        ))
    }
);

expand_proto!(
    expand_furnace,
    FurnacesInput,
    entries,
    "Furnaces",
    "register_furnaces",
    |e: &FurnaceEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        Ok(format!(
            "Furnace {{ name: \"{}\", crafting_speed: {}, crafting_categories: {}, energy_usage: \"{}\", energy_source: {}, result_inventory_size: {}, source_inventory_size: {}, icon: {icon}, module_slots: {}, subgroup: {}, order: {}, flags: {}, max_health: {}, ..Default::default() }}",
            e.name,
            e.crafting_speed,
            str_list_src(&e.crafting_categories),
            e.energy_usage,
            energy_source_src(&e.energy_type, e.usage_priority.as_deref()),
            e.result_inventory_size,
            e.source_inventory_size,
            option_i64_src(e.module_slots),
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
            option_flags_src(e.flags.as_deref()),
            option_f64_src(e.max_health),
        ))
    }
);

expand_proto!(
    expand_mining_drill,
    MiningDrillsInput,
    entries,
    "MiningDrills",
    "register_mining_drills",
    |e: &MiningDrillEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        Ok(format!(
            "MiningDrill {{ name: \"{}\", mining_speed: {}, energy_usage: \"{}\", energy_source: {}, resource_categories: {}, resource_searching_radius: {}, icon: {icon}, module_slots: {}, subgroup: {}, order: {}, flags: {}, ..Default::default() }}",
            e.name,
            e.mining_speed,
            e.energy_usage,
            energy_source_src(&e.energy_type, e.usage_priority.as_deref()),
            str_list_src(&e.resource_categories),
            e.resource_searching_radius,
            option_i64_src(e.module_slots),
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
            option_flags_src(e.flags.as_deref()),
        ))
    }
);

expand_proto!(
    expand_lab,
    LabsInput,
    entries,
    "Labs",
    "register_labs",
    |e: &LabEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        Ok(format!(
            "Lab {{ name: \"{}\", energy_usage: \"{}\", energy_source: {}, inputs: {}, icon: {icon}, module_slots: {}, subgroup: {}, order: {}, flags: {}, max_health: {}, ..Default::default() }}",
            e.name,
            e.energy_usage,
            energy_source_src(&e.energy_type, e.usage_priority.as_deref()),
            str_list_src(&e.inputs),
            option_i64_src(e.module_slots),
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
            option_flags_src(e.flags.as_deref()),
            option_f64_src(e.max_health),
        ))
    }
);

expand_proto!(
    expand_resource,
    ResourcesInput,
    entries,
    "Resources",
    "register_resources",
    |e: &ResourceEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        Ok(format!(
            "ResourceEntity {{ name: \"{}\", icon: {icon}, subgroup: {}, order: {}, flags: {}, ..Default::default() }}",
            e.name,
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
            option_flags_src(e.flags.as_deref()),
        ))
    }
);

expand_proto!(
    expand_tile,
    TilesInput,
    entries,
    "Tiles",
    "register_tiles",
    |e: &TileEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        Ok(format!(
            "Tile {{ name: \"{}\", layer: {}, map_color: {}, icon: {icon}, subgroup: {}, order: {}, ..Default::default() }}",
            e.name,
            e.layer,
            color_src(e.map_color.r, e.map_color.g, e.map_color.b, e.map_color.a),
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
        ))
    }
);

expand_proto!(
    expand_autoplace_control,
    AutoplaceControlsInput,
    entries,
    "AutoplaceControls",
    "register_autoplace_controls",
    |e: &AutoplaceControlEntry, _mod_name: Option<&str>| -> FrontendResult<String> {
        Ok(format!(
            "AutoplaceControl {{ name: \"{}\", category: \"{}\", order: {}, hidden: {}, ..Default::default() }}",
            e.name,
            e.category,
            option_str_src(e.order.as_deref()),
            option_bool_src(e.hidden),
        ))
    }
);

expand_proto!(
    expand_recipe_category,
    RecipeCategoriesInput,
    entries,
    "RecipeCategories",
    "register_recipe_categories",
    |e: &RecipeCategoryEntry, _mod_name: Option<&str>| -> FrontendResult<String> {
        Ok(format!(
            "RecipeCategory {{ name: \"{}\", order: {}, hidden: {}, ..Default::default() }}",
            e.name,
            option_str_src(e.order.as_deref()),
            option_bool_src(e.hidden),
        ))
    }
);

expand_proto!(
    expand_item_group,
    ItemGroupsInput,
    entries,
    "ItemGroups",
    "register_item_groups",
    |e: &ItemGroupEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        Ok(format!(
            "ItemGroup {{ name: \"{}\", icon: {icon}, order: {}, order_in_recipe: {}, ..Default::default() }}",
            e.name,
            option_str_src(e.order.as_deref()),
            option_str_src(e.order_in_recipe.as_deref()),
        ))
    }
);

expand_proto!(
    expand_item_subgroup,
    ItemSubgroupsInput,
    entries,
    "ItemSubgroups",
    "register_item_subgroups",
    |e: &ItemSubgroupEntry, _mod_name: Option<&str>| -> FrontendResult<String> {
        Ok(format!(
            "ItemSubgroup {{ name: \"{}\", group: \"{}\", order: {}, ..Default::default() }}",
            e.name,
            e.group,
            option_str_src(e.order.as_deref()),
        ))
    }
);

expand_proto!(
    expand_module,
    ModulesInput,
    entries,
    "Modules",
    "register_modules",
    |e: &ModuleEntry, mod_name: Option<&str>| -> FrontendResult<String> {
        let icon = option_str_src(resolve_optional_icon(e.icon.as_deref(), mod_name)?.as_deref());
        Ok(format!(
            "Module {{ name: \"{}\", stack_size: {}, category: \"{}\", tier: {}, icon: {icon}, subgroup: {}, order: {}, ..Default::default() }}",
            e.name,
            e.stack_size,
            e.category,
            e.tier,
            option_str_src(e.subgroup.as_deref()),
            option_str_src(e.order.as_deref()),
        ))
    }
);

fn parse_f64(input: ParseStream<'_>) -> syn::Result<f64> {
    if input.peek(LitFloat) {
        let lit: LitFloat = input.parse()?;
        lit.base10_parse()
    } else {
        let lit: LitInt = input.parse()?;
        lit.base10_parse()
    }
}

fn parse_str_list(input: ParseStream<'_>) -> syn::Result<Vec<String>> {
    let content;
    syn::bracketed!(content in input);
    let mut items = Vec::new();
    while !content.is_empty() {
        let lit: LitStr = content.parse()?;
        items.push(lit.value());
        let _: Option<Token![,]> = content.parse()?;
    }
    Ok(items)
}

fn parse_color(input: ParseStream<'_>) -> syn::Result<ColorLit> {
    let content;
    syn::braced!(content in input);
    let mut r = None;
    let mut g = None;
    let mut b = None;
    let mut a = None;
    while !content.is_empty() {
        let field: Ident = content.parse()?;
        let _: Token![=] = content.parse()?;
        match field.to_string().as_str() {
            "r" => r = Some(parse_f64(&content)?),
            "g" => g = Some(parse_f64(&content)?),
            "b" => b = Some(parse_f64(&content)?),
            "a" => a = Some(parse_f64(&content)?),
            other => {
                return Err(syn::Error::new(
                    field.span(),
                    format!("unknown color field `{other}`"),
                ));
            }
        }
        let _: Option<Token![,]> = content.parse()?;
    }
    Ok(ColorLit {
        r: r.ok_or_else(|| syn::Error::new(content.span(), "missing `r`"))?,
        g: g.ok_or_else(|| syn::Error::new(content.span(), "missing `g`"))?,
        b: b.ok_or_else(|| syn::Error::new(content.span(), "missing `b`"))?,
        a,
    })
}

#[derive(Clone, Copy)]
struct ColorLit {
    r: f64,
    g: f64,
    b: f64,
    a: Option<f64>,
}

macro_rules! proto_list_input {
    ($input:ident, $entry:ident, $msg:literal) => {
        struct $input {
            entries: Vec<$entry>,
        }

        impl Parse for $input {
            fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
                let mut entries = Vec::new();
                while !input.is_empty() {
                    entries.push(input.parse()?);
                }
                if entries.is_empty() {
                    return Err(input.error($msg));
                }
                Ok(Self { entries })
            }
        }
    };
}

proto_list_input!(
    ContainersInput,
    ContainerEntry,
    "expected at least one container block"
);
proto_list_input!(
    InsertersInput,
    InserterEntry,
    "expected at least one inserter block"
);
proto_list_input!(
    TransportBeltsInput,
    TransportBeltEntry,
    "expected at least one transport_belt block"
);
proto_list_input!(
    FurnacesInput,
    FurnaceEntry,
    "expected at least one furnace block"
);
proto_list_input!(
    MiningDrillsInput,
    MiningDrillEntry,
    "expected at least one mining_drill block"
);
proto_list_input!(LabsInput, LabEntry, "expected at least one lab block");
proto_list_input!(
    ResourcesInput,
    ResourceEntry,
    "expected at least one resource block"
);
proto_list_input!(TilesInput, TileEntry, "expected at least one tile block");
proto_list_input!(
    AutoplaceControlsInput,
    AutoplaceControlEntry,
    "expected at least one autoplace_control block"
);
proto_list_input!(
    RecipeCategoriesInput,
    RecipeCategoryEntry,
    "expected at least one recipe_category block"
);
proto_list_input!(
    ItemGroupsInput,
    ItemGroupEntry,
    "expected at least one item_group block"
);
proto_list_input!(
    ItemSubgroupsInput,
    ItemSubgroupEntry,
    "expected at least one item_subgroup block"
);
proto_list_input!(
    ModulesInput,
    ModuleEntry,
    "expected at least one module block"
);

struct ContainerEntry {
    ident: Ident,
    name: String,
    inventory_size: i64,
    icon: Option<String>,
    icon_size: Option<i64>,
    subgroup: Option<String>,
    order: Option<String>,
    flags: Option<Vec<String>>,
    max_health: Option<f64>,
}

impl Parse for ContainerEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut inventory_size = None;
        let mut icon = None;
        let mut icon_size = None;
        let mut subgroup = None;
        let mut order = None;
        let mut flags = None;
        let mut max_health = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "inventory_size" => {
                    let lit: LitInt = content.parse()?;
                    inventory_size = Some(lit.base10_parse()?);
                }
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "icon_size" => {
                    let lit: LitInt = content.parse()?;
                    icon_size = Some(lit.base10_parse()?);
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "flags" => flags = Some(parse_str_list(&content)?),
                "max_health" => max_health = Some(parse_f64(&content)?),
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown container field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            inventory_size: inventory_size
                .ok_or_else(|| syn::Error::new(span, "missing `inventory_size`"))?,
            icon,
            icon_size,
            subgroup,
            order,
            flags,
            max_health,
        })
    }
}

struct InserterEntry {
    ident: Ident,
    name: String,
    extension_speed: f64,
    rotation_speed: f64,
    energy_type: String,
    energy_usage: String,
    usage_priority: Option<String>,
    icon: Option<String>,
    subgroup: Option<String>,
    order: Option<String>,
    flags: Option<Vec<String>>,
    max_health: Option<f64>,
}

impl Parse for InserterEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut extension_speed = None;
        let mut rotation_speed = None;
        let mut energy_type = None;
        let mut energy_usage = None;
        let mut usage_priority = None;
        let mut icon = None;
        let mut subgroup = None;
        let mut order = None;
        let mut flags = None;
        let mut max_health = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "extension_speed" => extension_speed = Some(parse_f64(&content)?),
                "rotation_speed" => rotation_speed = Some(parse_f64(&content)?),
                "energy_type" => {
                    let lit: LitStr = content.parse()?;
                    energy_type = Some(lit.value());
                }
                "energy_usage" => {
                    let lit: LitStr = content.parse()?;
                    energy_usage = Some(lit.value());
                }
                "usage_priority" => {
                    let lit: LitStr = content.parse()?;
                    usage_priority = Some(lit.value());
                }
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "flags" => flags = Some(parse_str_list(&content)?),
                "max_health" => max_health = Some(parse_f64(&content)?),
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown inserter field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            extension_speed: extension_speed
                .ok_or_else(|| syn::Error::new(span, "missing `extension_speed`"))?,
            rotation_speed: rotation_speed
                .ok_or_else(|| syn::Error::new(span, "missing `rotation_speed`"))?,
            energy_type: energy_type
                .ok_or_else(|| syn::Error::new(span, "missing `energy_type`"))?,
            energy_usage: energy_usage.unwrap_or_else(|| "5kW".to_string()),
            usage_priority,
            icon,
            subgroup,
            order,
            flags,
            max_health,
        })
    }
}

struct TransportBeltEntry {
    ident: Ident,
    name: String,
    speed: f64,
    icon: Option<String>,
    subgroup: Option<String>,
    order: Option<String>,
    flags: Option<Vec<String>>,
    max_health: Option<f64>,
}

impl Parse for TransportBeltEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut speed = None;
        let mut icon = None;
        let mut subgroup = None;
        let mut order = None;
        let mut flags = None;
        let mut max_health = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "speed" => speed = Some(parse_f64(&content)?),
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "flags" => flags = Some(parse_str_list(&content)?),
                "max_health" => max_health = Some(parse_f64(&content)?),
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown transport_belt field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            speed: speed.ok_or_else(|| syn::Error::new(span, "missing `speed`"))?,
            icon,
            subgroup,
            order,
            flags,
            max_health,
        })
    }
}

struct FurnaceEntry {
    ident: Ident,
    name: String,
    crafting_speed: f64,
    crafting_categories: Vec<String>,
    energy_usage: String,
    energy_type: String,
    result_inventory_size: i64,
    source_inventory_size: i64,
    usage_priority: Option<String>,
    icon: Option<String>,
    module_slots: Option<i64>,
    subgroup: Option<String>,
    order: Option<String>,
    flags: Option<Vec<String>>,
    max_health: Option<f64>,
}

impl Parse for FurnaceEntry {
    #[allow(clippy::too_many_lines)]
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut crafting_speed = None;
        let mut crafting_categories = None;
        let mut energy_usage = None;
        let mut energy_type = None;
        let mut result_inventory_size = None;
        let mut source_inventory_size = None;
        let mut usage_priority = None;
        let mut icon = None;
        let mut module_slots = None;
        let mut subgroup = None;
        let mut order = None;
        let mut flags = None;
        let mut max_health = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "crafting_speed" => crafting_speed = Some(parse_f64(&content)?),
                "crafting_categories" => crafting_categories = Some(parse_str_list(&content)?),
                "energy_usage" => {
                    let lit: LitStr = content.parse()?;
                    energy_usage = Some(lit.value());
                }
                "energy_type" => {
                    let lit: LitStr = content.parse()?;
                    energy_type = Some(lit.value());
                }
                "result_inventory_size" => {
                    let lit: LitInt = content.parse()?;
                    result_inventory_size = Some(lit.base10_parse()?);
                }
                "source_inventory_size" => {
                    let lit: LitInt = content.parse()?;
                    source_inventory_size = Some(lit.base10_parse()?);
                }
                "usage_priority" => {
                    let lit: LitStr = content.parse()?;
                    usage_priority = Some(lit.value());
                }
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "module_slots" => {
                    let lit: LitInt = content.parse()?;
                    module_slots = Some(lit.base10_parse()?);
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "flags" => flags = Some(parse_str_list(&content)?),
                "max_health" => max_health = Some(parse_f64(&content)?),
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown furnace field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            crafting_speed: crafting_speed
                .ok_or_else(|| syn::Error::new(span, "missing `crafting_speed`"))?,
            crafting_categories: crafting_categories
                .ok_or_else(|| syn::Error::new(span, "missing `crafting_categories`"))?,
            energy_usage: energy_usage
                .ok_or_else(|| syn::Error::new(span, "missing `energy_usage`"))?,
            energy_type: energy_type
                .ok_or_else(|| syn::Error::new(span, "missing `energy_type`"))?,
            result_inventory_size: result_inventory_size
                .ok_or_else(|| syn::Error::new(span, "missing `result_inventory_size`"))?,
            source_inventory_size: source_inventory_size
                .ok_or_else(|| syn::Error::new(span, "missing `source_inventory_size`"))?,
            usage_priority,
            icon,
            module_slots,
            subgroup,
            order,
            flags,
            max_health,
        })
    }
}

struct MiningDrillEntry {
    ident: Ident,
    name: String,
    mining_speed: f64,
    energy_usage: String,
    energy_type: String,
    resource_categories: Vec<String>,
    resource_searching_radius: f64,
    usage_priority: Option<String>,
    icon: Option<String>,
    module_slots: Option<i64>,
    subgroup: Option<String>,
    order: Option<String>,
    flags: Option<Vec<String>>,
}

impl Parse for MiningDrillEntry {
    #[allow(clippy::too_many_lines)]
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut mining_speed = None;
        let mut energy_usage = None;
        let mut energy_type = None;
        let mut resource_categories = None;
        let mut resource_searching_radius = None;
        let mut usage_priority = None;
        let mut icon = None;
        let mut module_slots = None;
        let mut subgroup = None;
        let mut order = None;
        let mut flags = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "mining_speed" => mining_speed = Some(parse_f64(&content)?),
                "energy_usage" => {
                    let lit: LitStr = content.parse()?;
                    energy_usage = Some(lit.value());
                }
                "energy_type" => {
                    let lit: LitStr = content.parse()?;
                    energy_type = Some(lit.value());
                }
                "resource_categories" => resource_categories = Some(parse_str_list(&content)?),
                "resource_searching_radius" => {
                    resource_searching_radius = Some(parse_f64(&content)?);
                }
                "usage_priority" => {
                    let lit: LitStr = content.parse()?;
                    usage_priority = Some(lit.value());
                }
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "module_slots" => {
                    let lit: LitInt = content.parse()?;
                    module_slots = Some(lit.base10_parse()?);
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "flags" => flags = Some(parse_str_list(&content)?),
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown mining_drill field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            mining_speed: mining_speed
                .ok_or_else(|| syn::Error::new(span, "missing `mining_speed`"))?,
            energy_usage: energy_usage
                .ok_or_else(|| syn::Error::new(span, "missing `energy_usage`"))?,
            energy_type: energy_type
                .ok_or_else(|| syn::Error::new(span, "missing `energy_type`"))?,
            resource_categories: resource_categories
                .ok_or_else(|| syn::Error::new(span, "missing `resource_categories`"))?,
            resource_searching_radius: resource_searching_radius
                .ok_or_else(|| syn::Error::new(span, "missing `resource_searching_radius`"))?,
            usage_priority,
            icon,
            module_slots,
            subgroup,
            order,
            flags,
        })
    }
}

struct LabEntry {
    ident: Ident,
    name: String,
    energy_usage: String,
    energy_type: String,
    inputs: Vec<String>,
    usage_priority: Option<String>,
    icon: Option<String>,
    module_slots: Option<i64>,
    subgroup: Option<String>,
    order: Option<String>,
    flags: Option<Vec<String>>,
    max_health: Option<f64>,
}

impl Parse for LabEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut energy_usage = None;
        let mut energy_type = None;
        let mut inputs = None;
        let mut usage_priority = None;
        let mut icon = None;
        let mut module_slots = None;
        let mut subgroup = None;
        let mut order = None;
        let mut flags = None;
        let mut max_health = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "energy_usage" => {
                    let lit: LitStr = content.parse()?;
                    energy_usage = Some(lit.value());
                }
                "energy_type" => {
                    let lit: LitStr = content.parse()?;
                    energy_type = Some(lit.value());
                }
                "inputs" => inputs = Some(parse_str_list(&content)?),
                "usage_priority" => {
                    let lit: LitStr = content.parse()?;
                    usage_priority = Some(lit.value());
                }
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "module_slots" => {
                    let lit: LitInt = content.parse()?;
                    module_slots = Some(lit.base10_parse()?);
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "flags" => flags = Some(parse_str_list(&content)?),
                "max_health" => max_health = Some(parse_f64(&content)?),
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown lab field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            energy_usage: energy_usage
                .ok_or_else(|| syn::Error::new(span, "missing `energy_usage`"))?,
            energy_type: energy_type
                .ok_or_else(|| syn::Error::new(span, "missing `energy_type`"))?,
            inputs: inputs.ok_or_else(|| syn::Error::new(span, "missing `inputs`"))?,
            usage_priority,
            icon,
            module_slots,
            subgroup,
            order,
            flags,
            max_health,
        })
    }
}

struct ResourceEntry {
    ident: Ident,
    name: String,
    icon: Option<String>,
    subgroup: Option<String>,
    order: Option<String>,
    flags: Option<Vec<String>>,
}

impl Parse for ResourceEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut icon = None;
        let mut subgroup = None;
        let mut order = None;
        let mut flags = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "flags" => flags = Some(parse_str_list(&content)?),
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown resource field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            icon,
            subgroup,
            order,
            flags,
        })
    }
}

struct TileEntry {
    ident: Ident,
    name: String,
    layer: i64,
    map_color: ColorLit,
    icon: Option<String>,
    subgroup: Option<String>,
    order: Option<String>,
}

impl Parse for TileEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut layer = None;
        let mut map_color = None;
        let mut red = None;
        let mut green = None;
        let mut blue = None;
        let mut alpha = None;
        let mut icon = None;
        let mut subgroup = None;
        let mut order = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "layer" => {
                    let lit: LitInt = content.parse()?;
                    layer = Some(lit.base10_parse()?);
                }
                "map_color" => map_color = Some(parse_color(&content)?),
                "r" => red = Some(parse_f64(&content)?),
                "g" => green = Some(parse_f64(&content)?),
                "b" => blue = Some(parse_f64(&content)?),
                "a" => alpha = Some(parse_f64(&content)?),
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown tile field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        let map_color = match map_color {
            Some(color) => color,
            None => ColorLit {
                r: red.ok_or_else(|| syn::Error::new(span, "missing `map_color` or `r`"))?,
                g: green.ok_or_else(|| syn::Error::new(span, "missing `map_color` or `g`"))?,
                b: blue.ok_or_else(|| syn::Error::new(span, "missing `map_color` or `b`"))?,
                a: alpha,
            },
        };
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            layer: layer.ok_or_else(|| syn::Error::new(span, "missing `layer`"))?,
            map_color,
            icon,
            subgroup,
            order,
        })
    }
}

struct AutoplaceControlEntry {
    ident: Ident,
    name: String,
    category: String,
    order: Option<String>,
    hidden: Option<bool>,
}

impl Parse for AutoplaceControlEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut category = None;
        let mut order = None;
        let mut hidden = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "category" => {
                    let lit: LitStr = content.parse()?;
                    category = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "hidden" => {
                    let lit: LitBool = content.parse()?;
                    hidden = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown autoplace_control field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            category: category.ok_or_else(|| syn::Error::new(span, "missing `category`"))?,
            order,
            hidden,
        })
    }
}

struct RecipeCategoryEntry {
    ident: Ident,
    name: String,
    order: Option<String>,
    hidden: Option<bool>,
}

impl Parse for RecipeCategoryEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut order = None;
        let mut hidden = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "hidden" => {
                    let lit: LitBool = content.parse()?;
                    hidden = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown recipe_category field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            order,
            hidden,
        })
    }
}

struct ItemGroupEntry {
    ident: Ident,
    name: String,
    icon: Option<String>,
    order: Option<String>,
    order_in_recipe: Option<String>,
}

impl Parse for ItemGroupEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut icon = None;
        let mut order = None;
        let mut order_in_recipe = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                "order_in_recipe" => {
                    let lit: LitStr = content.parse()?;
                    order_in_recipe = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown item_group field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            icon,
            order,
            order_in_recipe,
        })
    }
}

struct ItemSubgroupEntry {
    ident: Ident,
    name: String,
    group: String,
    order: Option<String>,
}

impl Parse for ItemSubgroupEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut group = None;
        let mut order = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "group" => {
                    let lit: LitStr = content.parse()?;
                    group = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown item_subgroup field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            group: group.ok_or_else(|| syn::Error::new(span, "missing `group`"))?,
            order,
        })
    }
}

struct ModuleEntry {
    ident: Ident,
    name: String,
    stack_size: i64,
    category: String,
    tier: i64,
    icon: Option<String>,
    subgroup: Option<String>,
    order: Option<String>,
}

impl Parse for ModuleEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut stack_size = None;
        let mut category = None;
        let mut tier = None;
        let mut icon = None;
        let mut subgroup = None;
        let mut order = None;
        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "stack_size" => {
                    let lit: LitInt = content.parse()?;
                    stack_size = Some(lit.base10_parse()?);
                }
                "category" => {
                    let lit: LitStr = content.parse()?;
                    category = Some(lit.value());
                }
                "tier" => {
                    let lit: LitInt = content.parse()?;
                    tier = Some(lit.base10_parse()?);
                }
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "subgroup" => {
                    let lit: LitStr = content.parse()?;
                    subgroup = Some(lit.value());
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown module field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            stack_size: stack_size.ok_or_else(|| syn::Error::new(span, "missing `stack_size`"))?,
            category: category.ok_or_else(|| syn::Error::new(span, "missing `category`"))?,
            tier: tier.ok_or_else(|| syn::Error::new(span, "missing `tier`"))?,
            icon,
            subgroup,
            order,
        })
    }
}
