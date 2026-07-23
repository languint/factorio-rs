use std::fmt::Write;

use proc_macro2::TokenStream;
use syn::{
    Ident, LitFloat, LitInt, LitStr, Token,
    parse::{Parse, ParseStream},
};

use crate::error::{FrontendError, FrontendResult};

/// Expand `assembling_machine! { ... }` into `AssemblingMachines` + register fn.
pub fn expand(tokens: TokenStream, mod_name: Option<&str>) -> FrontendResult<Vec<syn::Item>> {
    let input: AssemblingMachinesMacroInput = syn::parse2(tokens).map_err(FrontendError::from)?;

    let mut const_defs = String::new();
    let mut extend_items = String::new();

    for entry in &input.machines {
        let const_name = entry.ident.to_string().to_uppercase();
        let icon = resolve_icon_path(&entry.icon, mod_name)?;
        let categories = entry
            .crafting_categories
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let flags = entry.flags.as_ref().map_or_else(
            || "None".to_string(),
            |flags| {
                let inner = flags
                    .iter()
                    .map(|f| format!("\"{f}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Some(&[{inner}])")
            },
        );
        let usage_priority = option_str_src(entry.usage_priority.as_deref());
        let icon_size = option_i64_src(entry.icon_size);
        let max_health = option_f64_src(entry.max_health);
        let module_slots = option_i64_src(entry.module_slots);
        let subgroup = option_str_src(entry.subgroup.as_deref());
        let order = option_str_src(entry.order.as_deref());

        let _ = writeln!(
            const_defs,
            "pub const {const_name}: &'static str = \"{}\";",
            entry.name
        );
        let _ = writeln!(
            extend_items,
            "AssemblingMachine {{ name: \"{}\", icon: \"{icon}\", crafting_speed: {}, crafting_categories: &[{categories}], energy_usage: \"{}\", energy_source: EnergySource {{ r#type: \"{}\", usage_priority: {usage_priority}, ..Default::default() }}, icon_size: {icon_size}, flags: {flags}, minable: None, max_health: {max_health}, collision_box: None, selection_box: None, module_slots: {module_slots}, subgroup: {subgroup}, order: {order}, ..Default::default() }},",
            entry.name, entry.crafting_speed, entry.energy_usage, entry.energy_type,
        );
    }

    let code = format!(
        "pub struct AssemblingMachines; \
         impl AssemblingMachines {{ {const_defs} }} \
         pub fn register_assembling_machines() {{ data.extend([ {extend_items} ]); }}"
    );
    let file: syn::File = syn::parse_str(&code).map_err(FrontendError::from)?;
    Ok(file.items)
}

fn resolve_icon_path(icon: &str, mod_name: Option<&str>) -> FrontendResult<String> {
    if icon.starts_with("__") {
        return Ok(icon.to_string());
    }
    let Some(mod_name) = mod_name else {
        return Err(FrontendError::ItemIconNeedsModName {
            path: icon.to_string(),
        });
    };
    let trimmed = icon.strip_prefix("./").unwrap_or(icon);
    Ok(format!("__{mod_name}__/{trimmed}"))
}

fn option_i64_src(value: Option<i64>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some({v})"))
}

fn option_f64_src(value: Option<f64>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some({v})"))
}

fn option_str_src(value: Option<&str>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some(\"{v}\")"))
}

struct AssemblingMachinesMacroInput {
    machines: Vec<AssemblingMachineProtoEntry>,
}

struct AssemblingMachineProtoEntry {
    ident: Ident,
    name: String,
    icon: String,
    crafting_speed: f64,
    crafting_categories: Vec<String>,
    energy_usage: String,
    energy_type: String,
    usage_priority: Option<String>,
    icon_size: Option<i64>,
    flags: Option<Vec<String>>,
    max_health: Option<f64>,
    module_slots: Option<i64>,
    subgroup: Option<String>,
    order: Option<String>,
}

impl Parse for AssemblingMachinesMacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut machines = Vec::new();
        while !input.is_empty() {
            machines.push(input.parse()?);
        }
        if machines.is_empty() {
            return Err(input.error("expected at least one assembling_machine block"));
        }
        Ok(Self { machines })
    }
}

impl Parse for AssemblingMachineProtoEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut icon = None;
        let mut crafting_speed = None;
        let mut crafting_categories = None;
        let mut energy_usage = None;
        let mut energy_type = None;
        let mut usage_priority = None;
        let mut icon_size = None;
        let mut flags = None;
        let mut max_health = None;
        let mut module_slots = None;
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
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
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
                "usage_priority" => {
                    let lit: LitStr = content.parse()?;
                    usage_priority = Some(lit.value());
                }
                "icon_size" => {
                    let lit: LitInt = content.parse()?;
                    icon_size = Some(lit.base10_parse()?);
                }
                "flags" => flags = Some(parse_str_list(&content)?),
                "max_health" => max_health = Some(parse_f64(&content)?),
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
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown assembling_machine field `{other}`"),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }
        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| syn::Error::new(span, "missing `name`"))?,
            icon: icon.ok_or_else(|| syn::Error::new(span, "missing `icon`"))?,
            crafting_speed: crafting_speed
                .ok_or_else(|| syn::Error::new(span, "missing `crafting_speed`"))?,
            crafting_categories: crafting_categories
                .ok_or_else(|| syn::Error::new(span, "missing `crafting_categories`"))?,
            energy_usage: energy_usage
                .ok_or_else(|| syn::Error::new(span, "missing `energy_usage`"))?,
            energy_type: energy_type.unwrap_or_else(|| "electric".to_string()),
            usage_priority,
            icon_size,
            flags,
            max_health,
            module_slots,
            subgroup,
            order,
        })
    }
}

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
