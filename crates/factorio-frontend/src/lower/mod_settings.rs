use std::fmt::Write;

use proc_macro2::TokenStream;
use syn::{
    Expr, LitStr, Token, Type,
    parse::{Parse, ParseStream},
};

use crate::error::{FrontendError, FrontendResult};

pub fn expand(tokens: TokenStream) -> FrontendResult<Vec<syn::Item>> {
    let input: ModSettingsInput = syn::parse2(tokens).map_err(FrontendError::from)?;

    let mut const_defs = String::new();
    let mut extend_items = String::new();

    for group in &input.groups {
        let stage_str = match group.stage {
            SettingStage::Startup => "startup",
            SettingStage::RuntimeGlobal => "runtime-global",
            SettingStage::RuntimePerUser => "runtime-per-user",
        };

        for entry in &group.entries {
            let const_name = entry.ident.to_string().to_uppercase();
            let lua_name = build_lua_name(input.prefix.as_deref(), &entry.ident.to_string());
            let default_str = expr_to_string(&entry.default);

            let _ = writeln!(
                const_defs,
                "pub const {const_name}: &'static str = \"{lua_name}\";"
            );

            let (struct_name, extra_fields) = match entry.setting_type {
                SettingType::Bool => ("BoolSetting", String::new()),
                SettingType::Int => (
                    "IntSetting",
                    "minimum_value: None, maximum_value: None,".to_string(),
                ),
                SettingType::Double => (
                    "DoubleSetting",
                    "minimum_value: None, maximum_value: None,".to_string(),
                ),
                SettingType::Str => ("StringSetting", "hidden: false,".to_string()),
            };

            let _ = writeln!(
                extend_items,
                "{struct_name} {{ name: \"{lua_name}\", setting_type: \"{stage_str}\", default_value: {default_str}, {extra_fields} }},"
            );
        }
    }

    let code = format!(
        "pub struct Settings; \
         impl Settings {{ {const_defs} }} \
         pub fn register() {{ data.extend([ {extend_items} ]); }}"
    );

    let file: syn::File = syn::parse_str(&code).map_err(FrontendError::from)?;
    Ok(file.items)
}

fn expr_to_string(expr: &Expr) -> String {
    use quote::ToTokens as _;
    expr.to_token_stream().to_string()
}

struct ModSettingsInput {
    prefix: Option<String>,
    groups: Vec<SettingGroup>,
}

struct SettingGroup {
    stage: SettingStage,
    entries: Vec<SettingEntry>,
}

struct SettingEntry {
    ident: syn::Ident,
    setting_type: SettingType,
    default: Expr,
}

#[derive(Clone, Copy)]
enum SettingStage {
    Startup,
    RuntimeGlobal,
    RuntimePerUser,
}

#[derive(Clone, Copy)]
enum SettingType {
    Bool,
    Int,
    Double,
    Str,
}

impl Parse for ModSettingsInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut prefix: Option<String> = None;
        if input.peek(syn::Ident) {
            let fork = input.fork();
            let kw: syn::Ident = fork.parse()?;
            if kw == "prefix" && fork.peek(Token![=]) {
                let _: syn::Ident = input.parse()?;
                let _: Token![=] = input.parse()?;
                let lit: LitStr = input.parse()?;
                prefix = Some(lit.value());
                let _: Option<Token![,]> = input.parse()?;
            }
        }

        let mut groups = Vec::new();
        while !input.is_empty() {
            groups.push(input.parse::<SettingGroup>()?);
        }

        Ok(Self { prefix, groups })
    }
}

impl Parse for SettingGroup {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let stage_kw: syn::Ident = input.parse()?;
        let stage = match stage_kw.to_string().as_str() {
            "startup" => SettingStage::Startup,
            "runtime_global" => SettingStage::RuntimeGlobal,
            "runtime_per_user" => SettingStage::RuntimePerUser,
            other => {
                return Err(syn::Error::new(
                    stage_kw.span(),
                    format!(
                        "unknown setting stage `{other}`; expected `startup`, `runtime_global`, or `runtime_per_user`"
                    ),
                ));
            }
        };

        let content;
        syn::braced!(content in input);

        let mut entries = Vec::new();
        while !content.is_empty() {
            entries.push(content.parse::<SettingEntry>()?);
        }

        Ok(Self { stage, entries })
    }
}

impl Parse for SettingEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        let _: Token![:] = input.parse()?;
        let ty: Type = input.parse()?;
        let _: Token![=] = input.parse()?;
        let default: Expr = input.parse()?;
        let _: Option<Token![,]> = input.parse()?;

        let setting_type = type_to_setting_type(&ty).ok_or_else(|| {
            syn::Error::new_spanned(
                &ty,
                "unsupported setting type; use `bool`, `i64`, `f64`, or `&'static str`",
            )
        })?;

        Ok(Self {
            ident,
            setting_type,
            default,
        })
    }
}

fn type_to_setting_type(ty: &Type) -> Option<SettingType> {
    match ty {
        Type::Path(tp) => {
            let ident = tp.path.get_ident()?.to_string();
            match ident.as_str() {
                "bool" => Some(SettingType::Bool),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" => {
                    Some(SettingType::Int)
                }
                "f32" | "f64" => Some(SettingType::Double),
                "String" => Some(SettingType::Str),
                _ => None,
            }
        }
        Type::Reference(tr) => {
            if let Type::Path(tp) = tr.elem.as_ref()
                && tp.path.is_ident("str")
            {
                Some(SettingType::Str)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn build_lua_name(prefix: Option<&str>, snake: &str) -> String {
    let kebab = snake.replace('_', "-");
    match prefix {
        Some(p) => format!("{p}-{kebab}"),
        None => kebab,
    }
}
