use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::{
    Expr, ItemFn, LitStr, Path, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
};

/// Marks a control-stage function as a Factorio event handler.
///
/// The event is inferred from the handler name and first parameter type
/// (`OnBuiltEntityEvent`). Filters are validated at compile time via a generated
/// const expression.
///
/// # Examples
///
/// Without filter:
/// ```ignore
/// #[factorio_rs::event]
/// pub fn on_singleplayer_init(event: OnSingleplayerInitEvent) {}
/// ```
///
/// With filter (filter expression is type-checked at compile time):
/// ```ignore
/// #[factorio_rs::event(filter = [OnBuiltEntityFilter::type_("inserter")])]
/// pub fn on_built_entity(event: OnBuiltEntityEvent) {}
/// ```
#[proc_macro_attribute]
pub fn event(args: TokenStream, input: TokenStream) -> TokenStream {
    let event_args = parse_macro_input!(args as EventAttributeArgs);
    let function = parse_macro_input!(input as ItemFn);

    let marker_type = match (&event_args.event, event_marker_from_param(&function)) {
        (Some(path), _) => path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        (None, Some(marker)) => Some(marker),
        (None, None) => None,
    };

    let Some(type_name) = marker_type else {
        return syn::Error::new_spanned(
            &function.sig,
            "expected an event parameter such as `event: OnBuiltEntityEvent`",
        )
        .to_compile_error()
        .into();
    };

    let Some(event_name) = lookup_event_name(&type_name) else {
        let span = event_args
            .event
            .as_ref()
            .map_or_else(|| function.sig.span(), Spanned::span);
        return syn::Error::new(span, format!("unsupported event type `{type_name}`"))
            .to_compile_error()
            .into();
    };

    if let Some(filter) = &event_args.filter
        && lookup_event_filter_type(&type_name).is_none()
    {
        return syn::Error::new_spanned(
            filter,
            format!("event `{type_name}` does not support filters"),
        )
        .to_compile_error()
        .into();
    }

    if function.sig.ident != event_name {
        return syn::Error::new_spanned(
            &function.sig.ident,
            format!("event handler must be named `{event_name}`"),
        )
        .to_compile_error()
        .into();
    }

    let filter_check: TokenStream2 =
        event_args
            .filter
            .as_ref()
            .map_or_else(TokenStream2::new, |filter_expr| {
                quote::quote! {
                    const _: () = { let _ = #filter_expr; };
                }
            });

    TokenStream::from(quote::quote! {
        #[allow(dead_code)]
        #function

        #filter_check
    })
}

struct EventAttributeArgs {
    event: Option<Path>,
    filter: Option<Expr>,
}

impl Parse for EventAttributeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                event: None,
                filter: None,
            });
        }

        // `filter = [...]` without an explicit event type
        if input.peek(syn::Ident) && input.peek2(Token![=]) {
            let keyword: syn::Ident = input.parse()?;
            if keyword != "filter" {
                return Err(syn::Error::new(
                    keyword.span(),
                    "expected `filter` or an event type such as `OnBuiltEntity`",
                ));
            }
            input.parse::<Token![=]>()?;
            return Ok(Self {
                event: None,
                filter: Some(input.parse::<Expr>()?),
            });
        }

        let event: Path = input.parse()?;
        let filter = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let keyword: syn::Ident = input.parse()?;
            if keyword != "filter" {
                return Err(syn::Error::new(
                    keyword.span(),
                    "expected `filter` after event type",
                ));
            }
            input.parse::<Token![=]>()?;
            Some(input.parse::<Expr>()?)
        } else {
            None
        };

        Ok(Self {
            event: Some(event),
            filter,
        })
    }
}

fn event_marker_from_param(function: &ItemFn) -> Option<String> {
    let syn::FnArg::Typed(pat_type) = function.sig.inputs.first()? else {
        return None;
    };
    event_marker_from_type(&pat_type.ty)
}

fn event_marker_from_type(ty: &Type) -> Option<String> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    let segments = &type_path.path.segments;
    if segments.len() == 1 {
        let ident = segments[0].ident.to_string();
        return ident.strip_suffix("Event").map(str::to_string);
    }
    None
}

fn lookup_event_name(type_name: &str) -> Option<&'static str> {
    include!(concat!(env!("OUT_DIR"), "/event_lookup.rs"))
}

fn lookup_event_filter_type(type_name: &str) -> Option<&'static str> {
    include!(concat!(env!("OUT_DIR"), "/event_filter_lookup.rs"))
}

#[proc_macro_attribute]
pub fn settings(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn data(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn control(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Marks a file or inline `mod` as shared-stage code for transpilation.
///
/// Shared modules may be required by any other stage.
#[proc_macro_attribute]
pub fn shared(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Declares a settings-stage module from a block of items.
#[proc_macro]
pub fn settings_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("settings", input)
}

/// Declares a data/prototype-stage module from a block of items.
#[proc_macro]
pub fn data_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("data", input)
}

/// Declares a control/runtime-stage module from a block of items.
#[proc_macro]
pub fn control_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("control", input)
}

/// Declares a shared-stage module from a block of items.
#[proc_macro]
pub fn shared_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("shared", input)
}

fn wrap_stage_module(stage: &str, input: TokenStream) -> TokenStream {
    let module_name = syn::Ident::new(
        &format!("__factorio_{stage}"),
        proc_macro2::Span::call_site(),
    );
    let items = proc_macro2::TokenStream::from(input);
    TokenStream::from(quote::quote! {
        #[doc(hidden)]
        mod #module_name { #items }
    })
}

/// Declare mod settings in a single, concise block.
///
/// # Example
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// factorio_rs::mod_settings! {
///     prefix = "ms",
///
///     startup {
///         casual_mode: bool = false,
///         adjacency_enabled: bool = true,
///     }
///
///     runtime_global {
///         max_entities: i64 = 100,
///     }
/// }
/// ```
///
/// Access in control stage:
/// ```ignore
/// let enabled = settings.startup.get::<bool>(Settings::CASUAL_MODE);
/// ```
#[proc_macro]
pub fn mod_settings(input: TokenStream) -> TokenStream {
    let ModSettingsInput { prefix, groups } = parse_macro_input!(input as ModSettingsInput);

    // Collect all settings in order.
    let mut const_defs = Vec::<TokenStream2>::new();
    let mut extend_items = Vec::<TokenStream2>::new();

    for group in &groups {
        let setting_type_str = match group.stage {
            SettingStage::Startup => "startup",
            SettingStage::RuntimeGlobal => "runtime-global",
            SettingStage::RuntimePerUser => "runtime-per-user",
        };

        for entry in &group.entries {
            let const_name = screaming_to_const_ident(&entry.ident);
            let lua_name = build_lua_name(prefix.as_deref(), &entry.ident.to_string());
            let default_expr = &entry.default;

            // `pub const CASUAL_MODE: &'static str = "ms-casual-mode";`
            const_defs.push(quote::quote! {
                pub const #const_name: &'static str = #lua_name;
            });

            let lua_name_lit = lua_name.as_str();
            let item_expr = match entry.setting_type {
                SettingType::Bool => quote::quote! {
                    BoolSetting {
                        name: #lua_name_lit,
                        setting_type: #setting_type_str,
                        default_value: #default_expr,
                    }
                },
                SettingType::Int => quote::quote! {
                    IntSetting {
                        name: #lua_name_lit,
                        setting_type: #setting_type_str,
                        default_value: #default_expr,
                        minimum_value: None,
                        maximum_value: None,
                    }
                },
                SettingType::Double => quote::quote! {
                    DoubleSetting {
                        name: #lua_name_lit,
                        setting_type: #setting_type_str,
                        default_value: #default_expr,
                        minimum_value: None,
                        maximum_value: None,
                    }
                },
                SettingType::Str => quote::quote! {
                    StringSetting {
                        name: #lua_name_lit,
                        setting_type: #setting_type_str,
                        default_value: #default_expr,
                        hidden: false,
                    }
                },
            };
            extend_items.push(item_expr);
        }
    }

    TokenStream::from(quote::quote! {
        pub struct Settings;

        impl Settings {
            #( #const_defs )*
        }

        pub fn register() {
            data.extend([
                #( #extend_items, )*
            ]);
        }
    })
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
        // &'static str or &str
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

/// Convert a `snake_case` ident to a `proc_macro2::Ident`.
fn screaming_to_const_ident(ident: &syn::Ident) -> proc_macro2::Ident {
    let upper = ident.to_string().to_uppercase();
    proc_macro2::Ident::new(&upper, ident.span())
}

/// Build the Lua setting name: `{prefix}-{kebab-case}` or just `{kebab-case}`.
fn build_lua_name(prefix: Option<&str>, snake: &str) -> String {
    let kebab = snake.replace('_', "-");
    match prefix {
        Some(p) => format!("{p}-{kebab}"),
        None => kebab,
    }
}
