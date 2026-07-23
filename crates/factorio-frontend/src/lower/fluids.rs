use std::fmt::Write;

use proc_macro2::TokenStream;
use syn::{
    Ident, LitBool, LitFloat, LitInt, LitStr, Token,
    parse::{Parse, ParseStream},
};

use crate::error::{FrontendError, FrontendResult};

/// Expand `fluid! { ... }` into `Fluids` + `pub fn register_fluids()`.
pub fn expand(tokens: TokenStream, mod_name: Option<&str>) -> FrontendResult<Vec<syn::Item>> {
    let input: FluidsMacroInput = syn::parse2(tokens).map_err(FrontendError::from)?;

    let mut const_defs = String::new();
    let mut extend_items = String::new();

    for entry in &input.fluids {
        let const_name = entry.ident.to_string().to_uppercase();
        let icon = resolve_icon_path(&entry.icon, mod_name)?;
        let icon_size = option_i64_src(entry.icon_size);
        let subgroup = option_str_src(entry.subgroup.as_deref());
        let order = option_str_src(entry.order.as_deref());
        let hidden = option_bool_src(entry.hidden);
        let ba = option_f64_src(entry.base_color.a);
        let fa = option_f64_src(entry.flow_color.a);

        let _ = writeln!(
            const_defs,
            "pub const {const_name}: &'static str = \"{}\";",
            entry.name
        );
        let _ = writeln!(
            extend_items,
            "Fluid {{ name: \"{}\", icon: \"{icon}\", default_temperature: {}, base_color: Color {{ r: {}, g: {}, b: {}, a: {ba}, ..Default::default() }}, flow_color: Color {{ r: {}, g: {}, b: {}, a: {fa}, ..Default::default() }}, icon_size: {icon_size}, subgroup: {subgroup}, order: {order}, hidden: {hidden}, ..Default::default() }},",
            entry.name,
            entry.default_temperature,
            entry.base_color.r,
            entry.base_color.g,
            entry.base_color.b,
            entry.flow_color.r,
            entry.flow_color.g,
            entry.flow_color.b,
        );
    }

    let code = format!(
        "pub struct Fluids; \
         impl Fluids {{ {const_defs} }} \
         pub fn register_fluids() {{ data.extend([ {extend_items} ]); }}"
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

fn option_bool_src(value: Option<bool>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some({v})"))
}

fn option_str_src(value: Option<&str>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some(\"{v}\")"))
}

struct FluidsMacroInput {
    fluids: Vec<FluidProtoEntry>,
}

struct FluidProtoEntry {
    ident: Ident,
    name: String,
    icon: String,
    default_temperature: f64,
    base_color: ColorLit,
    flow_color: ColorLit,
    icon_size: Option<i64>,
    subgroup: Option<String>,
    order: Option<String>,
    hidden: Option<bool>,
}

struct ColorLit {
    r: f64,
    g: f64,
    b: f64,
    a: Option<f64>,
}

impl Parse for FluidsMacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut fluids = Vec::new();
        while !input.is_empty() {
            fluids.push(input.parse()?);
        }
        if fluids.is_empty() {
            return Err(input.error("expected at least one fluid block"));
        }
        Ok(Self { fluids })
    }
}

impl Parse for FluidProtoEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut name = None;
        let mut icon = None;
        let mut default_temperature = None;
        let mut base_color = None;
        let mut flow_color = None;
        let mut icon_size = None;
        let mut subgroup = None;
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
                "icon" => {
                    let lit: LitStr = content.parse()?;
                    icon = Some(lit.value());
                }
                "default_temperature" => default_temperature = Some(parse_f64(&content)?),
                "base_color" => base_color = Some(parse_color(&content)?),
                "flow_color" => flow_color = Some(parse_color(&content)?),
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
                "hidden" => {
                    let lit: LitBool = content.parse()?;
                    hidden = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("unknown fluid field `{other}`"),
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
            default_temperature: default_temperature
                .ok_or_else(|| syn::Error::new(span, "missing `default_temperature`"))?,
            base_color: base_color.ok_or_else(|| syn::Error::new(span, "missing `base_color`"))?,
            flow_color: flow_color.ok_or_else(|| syn::Error::new(span, "missing `flow_color`"))?,
            icon_size,
            subgroup,
            order,
            hidden,
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
