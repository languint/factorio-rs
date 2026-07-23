use std::fmt::Write;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{
    Expr, Ident, LitBool, LitFloat, LitInt, LitStr, Token,
    parse::{Parse, ParseStream},
};

use crate::error::{FrontendError, FrontendResult};

/// Expand `recipe! { ... }` into `Recipes` constants + `pub fn register_recipes()`.
pub fn expand(tokens: TokenStream) -> FrontendResult<Vec<syn::Item>> {
    let input: RecipesMacroInput = syn::parse2(tokens).map_err(FrontendError::from)?;

    let mut const_defs = String::new();
    let mut extend_items = String::new();

    for entry in &input.recipes {
        let const_name = entry.ident.to_string().to_uppercase();

        let _ = writeln!(
            const_defs,
            "pub const {const_name}: &'static str = \"{}\";",
            entry.name
        );

        let ingredients = entry
            .ingredients
            .iter()
            .map(|ing| {
                format!(
                    "RecipeIngredient {{ name: {}, amount: {}, fluid: {}, ..Default::default() }}",
                    ing.name.to_src(),
                    ing.amount,
                    ing.fluid
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let results = entry
            .results
            .iter()
            .map(|prod| {
                format!(
                    "RecipeProduct {{ name: {}, amount: {}, ..Default::default() }}",
                    prod.name.to_src(),
                    prod.amount
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        let energy_required = option_f64_src(entry.energy_required);
        let category = option_str_src(entry.category.as_deref());
        let enabled = option_bool_src(entry.enabled);
        let subgroup = option_str_src(entry.subgroup.as_deref());
        let order = option_str_src(entry.order.as_deref());

        let _ = writeln!(
            extend_items,
            "Recipe {{ name: \"{}\", ingredients: &[{ingredients}], results: &[{results}], energy_required: {energy_required}, category: {category}, enabled: {enabled}, subgroup: {subgroup}, order: {order}, ..Default::default() }},",
            entry.name,
        );
    }

    let code = format!(
        "pub struct Recipes; \
         impl Recipes {{ {const_defs} }} \
         pub fn register_recipes() {{ data.extend([ {extend_items} ]); }}"
    );

    let file: syn::File = syn::parse_str(&code).map_err(FrontendError::from)?;
    Ok(file.items)
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

pub enum ProtoName {
    Lit(String),
    Path(syn::Path),
}

impl ProtoName {
    pub fn to_src(&self) -> String {
        match self {
            Self::Lit(s) => format!("\"{s}\""),
            Self::Path(path) => path.to_token_stream().to_string().replace(' ', ""),
        }
    }

    pub fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let expr: Expr = input.parse()?;
        match expr {
            Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => Ok(Self::Lit(s.value())),
            Expr::Path(path) => Ok(Self::Path(path.path)),
            other => Err(syn::Error::new_spanned(
                other,
                "expected a string literal or path (e.g. `Items::WIDGET`)",
            )),
        }
    }
}

struct RecipesMacroInput {
    recipes: Vec<RecipeProtoEntry>,
}

struct RecipeProtoEntry {
    ident: Ident,
    name: String,
    ingredients: Vec<RecipeComponent>,
    results: Vec<RecipeComponent>,
    energy_required: Option<f64>,
    category: Option<String>,
    enabled: Option<bool>,
    subgroup: Option<String>,
    order: Option<String>,
}

struct RecipeComponent {
    name: ProtoName,
    amount: i64,
    fluid: bool,
}

impl Parse for RecipesMacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut recipes = Vec::new();
        while !input.is_empty() {
            recipes.push(input.parse::<RecipeProtoEntry>()?);
        }
        if recipes.is_empty() {
            return Err(input.error("expected at least one recipe block"));
        }
        Ok(Self { recipes })
    }
}

impl Parse for RecipeProtoEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);

        let mut name: Option<String> = None;
        let mut ingredients: Option<Vec<RecipeComponent>> = None;
        let mut results: Option<Vec<RecipeComponent>> = None;
        let mut energy_required: Option<f64> = None;
        let mut category: Option<String> = None;
        let mut enabled: Option<bool> = None;
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
                "ingredients" => {
                    ingredients = Some(parse_recipe_components(&content)?);
                }
                "results" => {
                    results = Some(parse_recipe_components(&content)?);
                }
                "energy_required" => {
                    energy_required = Some(parse_f64_lit(&content)?);
                }
                "category" => {
                    let lit: LitStr = content.parse()?;
                    category = Some(lit.value());
                }
                "enabled" => {
                    let lit: LitBool = content.parse()?;
                    enabled = Some(lit.value());
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
                            "unknown recipe field `{other}`; expected `name`, `ingredients`, `results`, `energy_required`, `category`, `enabled`, `subgroup`, or `order`"
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
                syn::Error::new(span, "recipe block missing required field `name`")
            })?,
            ingredients: ingredients.ok_or_else(|| {
                syn::Error::new(span, "recipe block missing required field `ingredients`")
            })?,
            results: results.ok_or_else(|| {
                syn::Error::new(span, "recipe block missing required field `results`")
            })?,
            energy_required,
            category,
            enabled,
            subgroup,
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

fn parse_recipe_components(input: ParseStream<'_>) -> syn::Result<Vec<RecipeComponent>> {
    let content;
    syn::bracketed!(content in input);
    let mut components = Vec::new();
    while !content.is_empty() {
        components.push(content.parse::<RecipeComponent>()?);
        let _: Option<Token![,]> = content.parse()?;
    }
    Ok(components)
}

impl Parse for RecipeComponent {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);

        let mut name: Option<ProtoName> = None;
        let mut amount: Option<i64> = None;
        let mut fluid = false;

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
                "fluid" => {
                    let lit: LitBool = content.parse()?;
                    fluid = lit.value();
                }
                "type" => {
                    let lit: LitStr = content.parse()?;
                    match lit.value().as_str() {
                        "fluid" => fluid = true,
                        "item" => fluid = false,
                        other => {
                            return Err(syn::Error::new(
                                lit.span(),
                                format!(
                                    "unknown ingredient type `{other}`; expected `\"item\"` or `\"fluid\"`"
                                ),
                            ));
                        }
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!(
                            "unknown recipe component field `{other}`; expected `name`, `amount`, `fluid`, or `type`"
                        ),
                    ));
                }
            }
            let _: Option<Token![,]> = content.parse()?;
        }

        Ok(Self {
            name: name.ok_or_else(|| syn::Error::new(content.span(), "missing `name`"))?,
            amount: amount.ok_or_else(|| syn::Error::new(content.span(), "missing `amount`"))?,
            fluid,
        })
    }
}
