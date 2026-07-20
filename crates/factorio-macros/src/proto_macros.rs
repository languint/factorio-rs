//! Additional data-stage prototype proc macros.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::{
    Ident, LitBool, LitInt, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::proto_common::{
    ColorLit, color_tokens, emit_register_module, energy_source_tokens, option_bool_tokens,
    option_f64_tokens, option_flags_tokens, option_i64_tokens, option_icon_tokens,
    option_str_tokens, parse_color_lit, parse_f64_lit, parse_str_list, screaming_to_const_ident,
    str_list_tokens,
};

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

fn names_idents(names: &str, register: &str) -> (Ident, Ident) {
    (
        Ident::new(names, proc_macro2::Span::call_site()),
        Ident::new(register, proc_macro2::Span::call_site()),
    )
}

fn push_const(const_defs: &mut Vec<TokenStream2>, ident: &Ident, name: &str) {
    let const_name = screaming_to_const_ident(ident);
    const_defs.push(quote::quote! {
        pub const #const_name: &'static str = #name;
    });
}

pub fn container(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ContainersInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let inventory_size = entry.inventory_size;
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        let flags = option_flags_tokens(entry.flags.as_deref());
        let max_health = option_f64_tokens(entry.max_health);
        let _ = entry.icon_size;
        extend_items.push(quote::quote! {
            Container {
                name: #name,
                inventory_size: #inventory_size,
                icon: #icon,
                subgroup: #subgroup,
                order: #order,
                flags: #flags,
                max_health: #max_health,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("Containers", "register_containers");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn inserter(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as InsertersInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let extension_speed = entry.extension_speed;
        let rotation_speed = entry.rotation_speed;
        let energy_source =
            energy_source_tokens(&entry.energy_type, entry.usage_priority.as_deref());
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        let flags = option_flags_tokens(entry.flags.as_deref());
        let max_health = option_f64_tokens(entry.max_health);
        let _ = &entry.energy_usage;
        extend_items.push(quote::quote! {
            Inserter {
                name: #name,
                extension_speed: #extension_speed,
                rotation_speed: #rotation_speed,
                energy_source: #energy_source,
                icon: #icon,
                subgroup: #subgroup,
                order: #order,
                flags: #flags,
                max_health: #max_health,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("Inserters", "register_inserters");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn transport_belt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as TransportBeltsInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let speed = entry.speed;
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        let flags = option_flags_tokens(entry.flags.as_deref());
        let max_health = option_f64_tokens(entry.max_health);
        extend_items.push(quote::quote! {
            TransportBelt {
                name: #name,
                speed: #speed,
                icon: #icon,
                subgroup: #subgroup,
                order: #order,
                flags: #flags,
                max_health: #max_health,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("TransportBelts", "register_transport_belts");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn furnace(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as FurnacesInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let crafting_speed = entry.crafting_speed;
        let categories = str_list_tokens(&entry.crafting_categories);
        let energy_usage = entry.energy_usage.as_str();
        let energy_source =
            energy_source_tokens(&entry.energy_type, entry.usage_priority.as_deref());
        let result_inventory_size = entry.result_inventory_size;
        let source_inventory_size = entry.source_inventory_size;
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let module_slots = option_i64_tokens(entry.module_slots);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        let flags = option_flags_tokens(entry.flags.as_deref());
        let max_health = option_f64_tokens(entry.max_health);
        extend_items.push(quote::quote! {
            Furnace {
                name: #name,
                crafting_speed: #crafting_speed,
                crafting_categories: #categories,
                energy_usage: #energy_usage,
                energy_source: #energy_source,
                result_inventory_size: #result_inventory_size,
                source_inventory_size: #source_inventory_size,
                icon: #icon,
                module_slots: #module_slots,
                subgroup: #subgroup,
                order: #order,
                flags: #flags,
                max_health: #max_health,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("Furnaces", "register_furnaces");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn mining_drill(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as MiningDrillsInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let mining_speed = entry.mining_speed;
        let energy_usage = entry.energy_usage.as_str();
        let energy_source =
            energy_source_tokens(&entry.energy_type, entry.usage_priority.as_deref());
        let categories = str_list_tokens(&entry.resource_categories);
        let radius = entry.resource_searching_radius;
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let module_slots = option_i64_tokens(entry.module_slots);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        let flags = option_flags_tokens(entry.flags.as_deref());
        extend_items.push(quote::quote! {
            MiningDrill {
                name: #name,
                mining_speed: #mining_speed,
                energy_usage: #energy_usage,
                energy_source: #energy_source,
                resource_categories: #categories,
                resource_searching_radius: #radius,
                icon: #icon,
                module_slots: #module_slots,
                subgroup: #subgroup,
                order: #order,
                flags: #flags,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("MiningDrills", "register_mining_drills");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn lab(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LabsInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let energy_usage = entry.energy_usage.as_str();
        let energy_source =
            energy_source_tokens(&entry.energy_type, entry.usage_priority.as_deref());
        let inputs = str_list_tokens(&entry.inputs);
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let module_slots = option_i64_tokens(entry.module_slots);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        let flags = option_flags_tokens(entry.flags.as_deref());
        let max_health = option_f64_tokens(entry.max_health);
        extend_items.push(quote::quote! {
            Lab {
                name: #name,
                energy_usage: #energy_usage,
                energy_source: #energy_source,
                inputs: #inputs,
                icon: #icon,
                module_slots: #module_slots,
                subgroup: #subgroup,
                order: #order,
                flags: #flags,
                max_health: #max_health,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("Labs", "register_labs");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ResourcesInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        let flags = option_flags_tokens(entry.flags.as_deref());
        extend_items.push(quote::quote! {
            ResourceEntity {
                name: #name,
                icon: #icon,
                subgroup: #subgroup,
                order: #order,
                flags: #flags,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("Resources", "register_resources");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn tile(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as TilesInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let layer = entry.layer;
        let map_color = color_tokens(entry.map_color);
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        extend_items.push(quote::quote! {
            Tile {
                name: #name,
                layer: #layer,
                map_color: #map_color,
                icon: #icon,
                subgroup: #subgroup,
                order: #order,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("Tiles", "register_tiles");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn autoplace_control(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as AutoplaceControlsInput);
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let category = entry.category.as_str();
        let order = option_str_tokens(entry.order.as_deref());
        let hidden = option_bool_tokens(entry.hidden);
        extend_items.push(quote::quote! {
            AutoplaceControl {
                name: #name,
                category: #category,
                order: #order,
                hidden: #hidden,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("AutoplaceControls", "register_autoplace_controls");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn recipe_category(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as RecipeCategoriesInput);
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let order = option_str_tokens(entry.order.as_deref());
        let hidden = option_bool_tokens(entry.hidden);
        extend_items.push(quote::quote! {
            RecipeCategory {
                name: #name,
                order: #order,
                hidden: #hidden,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("RecipeCategories", "register_recipe_categories");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn item_group(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemGroupsInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let order = option_str_tokens(entry.order.as_deref());
        let order_in_recipe = option_str_tokens(entry.order_in_recipe.as_deref());
        extend_items.push(quote::quote! {
            ItemGroup {
                name: #name,
                icon: #icon,
                order: #order,
                order_in_recipe: #order_in_recipe,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("ItemGroups", "register_item_groups");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn item_subgroup(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemSubgroupsInput);
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let group = entry.group.as_str();
        let order = option_str_tokens(entry.order.as_deref());
        extend_items.push(quote::quote! {
            ItemSubgroup {
                name: #name,
                group: #group,
                order: #order,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("ItemSubgroups", "register_item_subgroups");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
}

pub fn module_proto(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ModulesInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());
    let mut const_defs = Vec::new();
    let mut extend_items = Vec::new();
    for entry in &input.entries {
        push_const(&mut const_defs, &entry.ident, &entry.name);
        let name = entry.name.as_str();
        let stack_size = entry.stack_size;
        let category = entry.category.as_str();
        let tier = entry.tier;
        let icon = option_icon_tokens(entry.icon.as_deref(), &mod_name);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());
        extend_items.push(quote::quote! {
            Module {
                name: #name,
                stack_size: #stack_size,
                category: #category,
                tier: #tier,
                icon: #icon,
                subgroup: #subgroup,
                order: #order,
                ..Default::default()
            }
        });
    }
    let (names, register) = names_idents("Modules", "register_modules");
    TokenStream::from(emit_register_module(
        &names,
        &register,
        &const_defs,
        &extend_items,
    ))
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
                "max_health" => max_health = Some(parse_f64_lit(&content)?),
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
                "extension_speed" => extension_speed = Some(parse_f64_lit(&content)?),
                "rotation_speed" => rotation_speed = Some(parse_f64_lit(&content)?),
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
                "max_health" => max_health = Some(parse_f64_lit(&content)?),
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
                "speed" => speed = Some(parse_f64_lit(&content)?),
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
                "max_health" => max_health = Some(parse_f64_lit(&content)?),
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
                "crafting_speed" => crafting_speed = Some(parse_f64_lit(&content)?),
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
                "max_health" => max_health = Some(parse_f64_lit(&content)?),
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
                "mining_speed" => mining_speed = Some(parse_f64_lit(&content)?),
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
                    resource_searching_radius = Some(parse_f64_lit(&content)?);
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
                "max_health" => max_health = Some(parse_f64_lit(&content)?),
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
                "map_color" => map_color = Some(parse_color_lit(&content)?),
                "r" => red = Some(parse_f64_lit(&content)?),
                "g" => green = Some(parse_f64_lit(&content)?),
                "b" => blue = Some(parse_f64_lit(&content)?),
                "a" => alpha = Some(parse_f64_lit(&content)?),
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
