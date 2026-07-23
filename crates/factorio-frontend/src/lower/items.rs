use std::fmt::Write;

use proc_macro2::TokenStream;
use syn::{
    Ident, LitInt, LitStr, Token,
    parse::{Parse, ParseStream},
};

use crate::error::{FrontendError, FrontendResult};

/// Expand `item! { ... }` into `Items` constants + `pub fn register()`.
pub fn expand(tokens: TokenStream, mod_name: Option<&str>) -> FrontendResult<Vec<syn::Item>> {
    let input: ItemsMacroInput = syn::parse2(tokens).map_err(FrontendError::from)?;

    let mut const_defs = String::new();
    let mut extend_items = String::new();

    for entry in &input.items {
        let const_name = entry.ident.to_string().to_uppercase();
        let icon = resolve_icon_path(&entry.icon, mod_name)?;

        let _ = writeln!(
            const_defs,
            "pub const {const_name}: &'static str = \"{}\";",
            entry.name
        );

        let icon_size = option_i64_src(entry.icon_size);
        let subgroup = option_str_src(entry.subgroup.as_deref());
        let order = option_str_src(entry.order.as_deref());

        let _ = writeln!(
            extend_items,
            "Item {{ name: \"{}\", icon: \"{icon}\", stack_size: {}, icon_size: {icon_size}, subgroup: {subgroup}, order: {order}, ..Default::default() }},",
            entry.name, entry.stack_size,
        );
    }

    let code = format!(
        "pub struct Items; \
         impl Items {{ {const_defs} }} \
         pub fn register() {{ data.extend([ {extend_items} ]); }}"
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

fn option_str_src(value: Option<&str>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some(\"{v}\")"))
}

struct ItemsMacroInput {
    items: Vec<ItemProtoEntry>,
}

struct ItemProtoEntry {
    ident: Ident,
    name: String,
    icon: String,
    stack_size: i64,
    icon_size: Option<i64>,
    subgroup: Option<String>,
    order: Option<String>,
}

impl Parse for ItemsMacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut items = Vec::new();
        while !input.is_empty() {
            items.push(input.parse::<ItemProtoEntry>()?);
        }
        if items.is_empty() {
            return Err(input.error("expected at least one item block"));
        }
        Ok(Self { items })
    }
}

impl Parse for ItemProtoEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);

        let mut name: Option<String> = None;
        let mut icon: Option<String> = None;
        let mut stack_size: Option<i64> = None;
        let mut icon_size: Option<i64> = None;
        let mut subgroup: Option<String> = None;
        let mut order: Option<String> = None;

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
                "stack_size" => {
                    let lit: LitInt = content.parse()?;
                    stack_size = Some(lit.base10_parse()?);
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
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!(
                            "unknown item field `{other}`; expected `name`, `icon`, `stack_size`, `icon_size`, `subgroup`, or `order`"
                        ),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }

        let span = ident.span();
        Ok(Self {
            ident,
            name: name
                .ok_or_else(|| syn::Error::new(span, "item block missing required field `name`"))?,
            icon: icon
                .ok_or_else(|| syn::Error::new(span, "item block missing required field `icon`"))?,
            stack_size: stack_size.ok_or_else(|| {
                syn::Error::new(span, "item block missing required field `stack_size`")
            })?,
            icon_size,
            subgroup,
            order,
        })
    }
}
