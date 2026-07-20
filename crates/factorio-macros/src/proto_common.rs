//! Shared helpers for new data-stage prototype proc macros.

use proc_macro2::TokenStream as TokenStream2;
use syn::{Ident, LitFloat, LitInt, LitStr, Token, parse::ParseStream};

pub fn screaming_to_const_ident(ident: &syn::Ident) -> proc_macro2::Ident {
    let upper = ident.to_string().to_uppercase();
    proc_macro2::Ident::new(&upper, ident.span())
}

pub fn resolve_icon_path(icon: &str, mod_name: &str) -> String {
    if icon.starts_with("__") {
        return icon.to_string();
    }
    let trimmed = icon.strip_prefix("./").unwrap_or(icon);
    format!("__{mod_name}__/{trimmed}")
}

pub fn option_i64_tokens(value: Option<i64>) -> TokenStream2 {
    value.map_or_else(|| quote::quote! { None }, |v| quote::quote! { Some(#v) })
}

pub fn option_f64_tokens(value: Option<f64>) -> TokenStream2 {
    value.map_or_else(|| quote::quote! { None }, |v| quote::quote! { Some(#v) })
}

pub fn option_bool_tokens(value: Option<bool>) -> TokenStream2 {
    value.map_or_else(|| quote::quote! { None }, |v| quote::quote! { Some(#v) })
}

pub fn option_str_tokens(value: Option<&str>) -> TokenStream2 {
    value.map_or_else(|| quote::quote! { None }, |v| quote::quote! { Some(#v) })
}

pub fn option_flags_tokens(flags: Option<&[String]>) -> TokenStream2 {
    flags.map_or_else(
        || quote::quote! { None },
        |flags| {
            let flags = flags.iter().map(|f| {
                let s = f.as_str();
                quote::quote! { #s }
            });
            quote::quote! { Some(&[ #( #flags ),* ]) }
        },
    )
}

pub fn str_list_tokens(items: &[String]) -> TokenStream2 {
    let items = items.iter().map(|s| {
        let s = s.as_str();
        quote::quote! { #s }
    });
    quote::quote! { &[ #( #items ),* ] }
}

pub fn energy_source_tokens(energy_type: &str, usage_priority: Option<&str>) -> TokenStream2 {
    let usage_priority = option_str_tokens(usage_priority);
    quote::quote! {
        EnergySource {
            r#type: #energy_type,
            usage_priority: #usage_priority,
            ..Default::default()
        }
    }
}

pub fn color_tokens(color: ColorLit) -> TokenStream2 {
    let ColorLit { r, g, b, a } = color;
    let a = option_f64_tokens(a);
    quote::quote! {
        Color {
            r: #r,
            g: #g,
            b: #b,
            a: #a,
            ..Default::default()
        }
    }
}

pub fn option_icon_tokens(icon: Option<&str>, mod_name: &str) -> TokenStream2 {
    icon.map_or_else(
        || quote::quote! { None },
        |path| {
            let resolved = resolve_icon_path(path, mod_name);
            quote::quote! { Some(#resolved) }
        },
    )
}

pub fn parse_f64_lit(input: ParseStream<'_>) -> syn::Result<f64> {
    if input.peek(LitFloat) {
        let lit: LitFloat = input.parse()?;
        lit.base10_parse()
    } else {
        let lit: LitInt = input.parse()?;
        lit.base10_parse()
    }
}

pub fn parse_str_list(input: ParseStream<'_>) -> syn::Result<Vec<String>> {
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

#[derive(Clone, Copy)]
pub struct ColorLit {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: Option<f64>,
}

pub fn parse_color_lit(input: ParseStream<'_>) -> syn::Result<ColorLit> {
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
            "r" => r = Some(parse_f64_lit(&content)?),
            "g" => g = Some(parse_f64_lit(&content)?),
            "b" => b = Some(parse_f64_lit(&content)?),
            "a" => a = Some(parse_f64_lit(&content)?),
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

pub fn emit_register_module(
    names_struct: &syn::Ident,
    register_fn: &syn::Ident,
    const_defs: &[TokenStream2],
    extend_items: &[TokenStream2],
) -> TokenStream2 {
    quote::quote! {
        pub struct #names_struct;

        impl #names_struct {
            #( #const_defs )*
        }

        pub fn #register_fn() {
            data.extend([
                #( #extend_items, )*
            ]);
        }
    }
}
