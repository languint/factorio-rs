use std::fmt::Write;

use proc_macro2::TokenStream;
use syn::{
    Ident, LitFloat, LitInt, LitStr, Token,
    parse::{Parse, ParseStream},
};

use crate::error::{FrontendError, FrontendResult};
use crate::lower::recipes::ProtoName;

/// Expand `technology! { ... }` into `Technologies` + `pub fn register_technologies()`.
pub fn expand(tokens: TokenStream, mod_name: Option<&str>) -> FrontendResult<Vec<syn::Item>> {
    let input: TechnologiesMacroInput =
        syn::parse2(tokens).map_err(FrontendError::from)?;

    let mut const_defs = String::new();
    let mut extend_items = String::new();

    for entry in &input.technologies {
        let const_name = entry.ident.to_string().to_uppercase();
        let icon = resolve_icon_path(&entry.icon, mod_name)?;

        let _ = writeln!(
            const_defs,
            "pub const {const_name}: &'static str = \"{}\";",
            entry.name
        );

        let prerequisites = entry
            .prerequisites
            .iter()
            .map(ProtoName::to_src)
            .collect::<Vec<_>>()
            .join(", ");
        let effects = entry
            .unlock_recipes
            .iter()
            .map(|r| {
                format!(
                    "UnlockRecipeEffect {{ recipe: {}, ..Default::default() }}",
                    r.to_src()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let unit_ingredients = entry
            .unit_ingredients
            .iter()
            .map(|ing| {
                format!(
                    "TechnologyUnitIngredient {{ name: {}, amount: {}, ..Default::default() }}",
                    ing.name.to_src(),
                    ing.amount
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        let icon_size = option_i64_src(entry.icon_size);
        let order = option_str_src(entry.order.as_deref());

        let _ = writeln!(
            extend_items,
            "Technology {{ name: \"{}\", icon: \"{icon}\", icon_size: {icon_size}, prerequisites: &[{prerequisites}], effects: &[{effects}], unit: TechnologyUnit {{ count: {}, time: {}, ingredients: &[{unit_ingredients}], ..Default::default() }}, order: {order}, ..Default::default() }},",
            entry.name, entry.unit_count, entry.unit_time,
        );
    }

    let code = format!(
        "pub struct Technologies; \
         impl Technologies {{ {const_defs} }} \
         pub fn register_technologies() {{ data.extend([ {extend_items} ]); }}"
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

struct TechnologiesMacroInput {
    technologies: Vec<TechnologyProtoEntry>,
}

struct TechnologyProtoEntry {
    ident: Ident,
    name: String,
    icon: String,
    icon_size: Option<i64>,
    prerequisites: Vec<ProtoName>,
    unlock_recipes: Vec<ProtoName>,
    unit_count: i64,
    unit_time: f64,
    unit_ingredients: Vec<UnitIngredient>,
    order: Option<String>,
}

struct UnitIngredient {
    name: ProtoName,
    amount: i64,
}

impl Parse for TechnologiesMacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut technologies = Vec::new();
        while !input.is_empty() {
            technologies.push(input.parse::<TechnologyProtoEntry>()?);
        }
        if technologies.is_empty() {
            return Err(input.error("expected at least one technology block"));
        }
        Ok(Self { technologies })
    }
}

impl Parse for TechnologyProtoEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);

        let mut name: Option<String> = None;
        let mut icon: Option<String> = None;
        let mut icon_size: Option<i64> = None;
        let mut prerequisites: Option<Vec<ProtoName>> = None;
        let mut unlock_recipes: Option<Vec<ProtoName>> = None;
        let mut unit_count: Option<i64> = None;
        let mut unit_time: Option<f64> = None;
        let mut unit_ingredients: Option<Vec<UnitIngredient>> = None;
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
                "icon_size" => {
                    let lit: LitInt = content.parse()?;
                    icon_size = Some(lit.base10_parse()?);
                }
                "prerequisites" => {
                    prerequisites = Some(parse_name_list(&content)?);
                }
                "unlock_recipes" => {
                    unlock_recipes = Some(parse_name_list(&content)?);
                }
                "unit_count" => {
                    let lit: LitInt = content.parse()?;
                    unit_count = Some(lit.base10_parse()?);
                }
                "unit_time" => {
                    unit_time = Some(parse_f64_lit(&content)?);
                }
                "unit_ingredients" => {
                    unit_ingredients = Some(parse_unit_ingredients(&content)?);
                }
                "order" => {
                    let lit: LitStr = content.parse()?;
                    order = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!(
                            "unknown technology field `{other}`; expected `name`, `icon`, `icon_size`, `prerequisites`, `unlock_recipes`, `unit_count`, `unit_time`, `unit_ingredients`, or `order`"
                        ),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }

        let span = ident.span();
        Ok(Self {
            ident,
            name: name.ok_or_else(|| {
                syn::Error::new(span, "technology block missing required field `name`")
            })?,
            icon: icon.ok_or_else(|| {
                syn::Error::new(span, "technology block missing required field `icon`")
            })?,
            icon_size,
            prerequisites: prerequisites.unwrap_or_default(),
            unlock_recipes: unlock_recipes.ok_or_else(|| {
                syn::Error::new(
                    span,
                    "technology block missing required field `unlock_recipes`",
                )
            })?,
            unit_count: unit_count.ok_or_else(|| {
                syn::Error::new(span, "technology block missing required field `unit_count`")
            })?,
            unit_time: unit_time.ok_or_else(|| {
                syn::Error::new(span, "technology block missing required field `unit_time`")
            })?,
            unit_ingredients: unit_ingredients.ok_or_else(|| {
                syn::Error::new(
                    span,
                    "technology block missing required field `unit_ingredients`",
                )
            })?,
            order,
        })
    }
}

fn parse_f64_lit(input: ParseStream<'_>) -> syn::Result<f64> {
    if input.peek(LitFloat) {
        let lit: LitFloat = input.parse()?;
        lit.base10_parse()
    } else {
        let lit: LitInt = input.parse()?;
        lit.base10_parse()
    }
}

fn parse_name_list(input: ParseStream<'_>) -> syn::Result<Vec<ProtoName>> {
    let content;
    syn::bracketed!(content in input);
    let mut items = Vec::new();
    while !content.is_empty() {
        items.push(ProtoName::parse(&content)?);
        let _: Option<Token![,]> = content.parse()?;
    }
    Ok(items)
}

fn parse_unit_ingredients(input: ParseStream<'_>) -> syn::Result<Vec<UnitIngredient>> {
    let content;
    syn::bracketed!(content in input);
    let mut ingredients = Vec::new();
    while !content.is_empty() {
        ingredients.push(content.parse::<UnitIngredient>()?);
        let _: Option<Token![,]> = content.parse()?;
    }
    Ok(ingredients)
}

impl Parse for UnitIngredient {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);

        let mut name: Option<ProtoName> = None;
        let mut amount: Option<i64> = None;

        while !content.is_empty() {
            let field: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    name = Some(ProtoName::parse(&content)?);
                }
                "amount" => {
                    let lit: LitInt = content.parse()?;
                    amount = Some(lit.base10_parse()?);
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!(
                            "unknown unit ingredient field `{other}`; expected `name` or `amount`"
                        ),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }

        Ok(Self {
            name: name.ok_or_else(|| syn::Error::new(content.span(), "missing `name`"))?,
            amount: amount.ok_or_else(|| syn::Error::new(content.span(), "missing `amount`"))?,
        })
    }
}
