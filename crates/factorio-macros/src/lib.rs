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
pub fn settings_updates(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn settings_final_fixes(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn data(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn data_updates(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn data_final_fixes(_args: TokenStream, input: TokenStream) -> TokenStream {
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

/// Publishes a function (or every `pub fn` in a module) as part of this mod's
/// cross-mod API.
///
/// Control-stage exports are registered with Factorio `remote.add_interface`.
/// Shared-stage exports remain requireable module functions and are included in
/// the generated `api/` stub crate.
///
/// Optional remote interface:
/// - `#[factorio_rs::export(interface)]` - remote using the mod name
/// - `#[factorio_rs::export(interface = "my_iface")]` - remote on a custom name
///
/// On a `mod` item, every public function inside inherits the export without
/// needing a per-fn attribute.
#[proc_macro_attribute]
pub fn export(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        let _ = parse_macro_input!(args as ExportAttributeArgs);
    }
    input
}

/// Parsed `#[export(...)]` interface argument (validation-only today).
#[allow(dead_code)]
enum ExportInterfaceArg {
    /// `#[export(interface)]` - remote using the mod-name default.
    Default,
    /// `#[export(interface = "name")]`.
    Named(LitStr),
}

struct ExportAttributeArgs {
    #[allow(dead_code)]
    interface: Option<ExportInterfaceArg>,
}

impl Parse for ExportAttributeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self { interface: None });
        }
        let keyword: syn::Ident = input.parse()?;
        if keyword != "interface" {
            return Err(syn::Error::new(
                keyword.span(),
                "expected `interface` or `interface = \"...\"`",
            ));
        }
        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Ok(Self {
                interface: Some(ExportInterfaceArg::Named(input.parse()?)),
            })
        } else {
            Ok(Self {
                interface: Some(ExportInterfaceArg::Default),
            })
        }
    }
}

/// Declares a settings-stage module from a block of items.
#[proc_macro]
pub fn settings_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("settings", input)
}

/// Declares a settings-updates-stage module from a block of items.
#[proc_macro]
pub fn settings_updates_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("settings_updates", input)
}

/// Declares a settings-final-fixes-stage module from a block of items.
#[proc_macro]
pub fn settings_final_fixes_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("settings_final_fixes", input)
}

/// Declares a data/prototype-stage module from a block of items.
#[proc_macro]
pub fn data_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("data", input)
}

/// Declares a data-updates-stage module from a block of items.
#[proc_macro]
pub fn data_updates_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("data_updates", input)
}

/// Declares a data-final-fixes-stage module from a block of items.
#[proc_macro]
pub fn data_final_fixes_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("data_final_fixes", input)
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

/// Declare data-stage item prototypes.
///
/// Expands to an `Items` type with name constants (for `locale!`) and
/// `pub fn register()` that calls `data.extend` with [`Item`] literals.
/// Relative `icon` paths are prefixed with `__{CARGO_PKG_NAME}__/`.
///
/// # Example
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// item! {
///     widget {
///         name = "my-mod-widget",
///         icon = "graphics/icon.png",
///         stack_size = 50,
///         icon_size = 64,
///     }
/// }
///
/// locale! {
///     en {
///         item_name {
///             Items::WIDGET = "Widget",
///         }
///     }
/// }
/// ```
#[proc_macro]
pub fn item(input: TokenStream) -> TokenStream {
    let ItemsMacroInput { items } = parse_macro_input!(input as ItemsMacroInput);
    let mod_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "mod".to_string());

    let mut const_defs = Vec::<TokenStream2>::new();
    let mut extend_items = Vec::<TokenStream2>::new();

    for entry in &items {
        let const_name = screaming_to_const_ident(&entry.ident);
        let name_lit = entry.name.as_str();
        let icon_lit = resolve_icon_path(&entry.icon, &mod_name);
        let stack_size = entry.stack_size;
        let icon_size = option_i64_tokens(entry.icon_size);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());

        const_defs.push(quote::quote! {
            pub const #const_name: &'static str = #name_lit;
        });

        extend_items.push(quote::quote! {
            Item {
                name: #name_lit,
                icon: #icon_lit,
                stack_size: #stack_size,
                icon_size: #icon_size,
                subgroup: #subgroup,
                order: #order,
                ..Default::default()
            }
        });
    }

    TokenStream::from(quote::quote! {
        pub struct Items;

        impl Items {
            #( #const_defs )*
        }

        pub fn register() {
            data.extend([
                #( #extend_items, )*
            ]);
        }
    })
}

fn resolve_icon_path(icon: &str, mod_name: &str) -> String {
    if icon.starts_with("__") {
        return icon.to_string();
    }
    let trimmed = icon.strip_prefix("./").unwrap_or(icon);
    format!("__{mod_name}__/{trimmed}")
}

fn option_i64_tokens(value: Option<i64>) -> TokenStream2 {
    value.map_or_else(|| quote::quote! { None }, |v| quote::quote! { Some(#v) })
}

fn option_str_tokens(value: Option<&str>) -> TokenStream2 {
    value.map_or_else(|| quote::quote! { None }, |v| quote::quote! { Some(#v) })
}

struct ItemsMacroInput {
    items: Vec<ItemProtoEntry>,
}

struct ItemProtoEntry {
    ident: syn::Ident,
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
        let ident: syn::Ident = input.parse()?;
        let content;
        syn::braced!(content in input);

        let mut name: Option<String> = None;
        let mut icon: Option<String> = None;
        let mut stack_size: Option<i64> = None;
        let mut icon_size: Option<i64> = None;
        let mut subgroup: Option<String> = None;
        let mut order: Option<String> = None;

        while !content.is_empty() {
            let field: syn::Ident = content.parse()?;
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
                    let lit: syn::LitInt = content.parse()?;
                    stack_size = Some(lit.base10_parse()?);
                }
                "icon_size" => {
                    let lit: syn::LitInt = content.parse()?;
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

/// Declare data-stage recipe prototypes.
///
/// Expands to a `Recipes` type with name constants (for `locale!`) and
/// `pub fn register_recipes()` that calls `data.extend` with [`Recipe`]
/// literals. Prefer `register_recipes` over `register` so `item!` and
/// `recipe!` can coexist in one module.
///
/// # Example
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// recipe! {
///     craft_widget {
///         name = "my-mod-widget",
///         energy_required = 1.0,
///         ingredients = [
///             { name = "iron-plate", amount = 2 },
///         ],
///         results = [
///             { name = "my-mod-widget", amount = 1 },
///         ],
///         category = "crafting",
///         enabled = true,
///     }
/// }
/// ```
#[proc_macro]
pub fn recipe(input: TokenStream) -> TokenStream {
    let RecipesMacroInput { recipes } = parse_macro_input!(input as RecipesMacroInput);

    let mut const_defs = Vec::<TokenStream2>::new();
    let mut extend_items = Vec::<TokenStream2>::new();

    for entry in &recipes {
        let const_name = screaming_to_const_ident(&entry.ident);
        let name_lit = entry.name.as_str();
        let energy_required = option_f64_tokens(entry.energy_required);
        let category = option_str_tokens(entry.category.as_deref());
        let enabled = option_bool_tokens(entry.enabled);
        let subgroup = option_str_tokens(entry.subgroup.as_deref());
        let order = option_str_tokens(entry.order.as_deref());

        let ingredients = entry.ingredients.iter().map(|ing| {
            let n = ing.name.as_str();
            let amount = ing.amount;
            quote::quote! {
                RecipeIngredient {
                    name: #n,
                    amount: #amount,
                    ..Default::default()
                }
            }
        });
        let results = entry.results.iter().map(|prod| {
            let n = prod.name.as_str();
            let amount = prod.amount;
            quote::quote! {
                RecipeProduct {
                    name: #n,
                    amount: #amount,
                    ..Default::default()
                }
            }
        });

        const_defs.push(quote::quote! {
            pub const #const_name: &'static str = #name_lit;
        });

        extend_items.push(quote::quote! {
            Recipe {
                name: #name_lit,
                ingredients: &[ #( #ingredients ),* ],
                results: &[ #( #results ),* ],
                energy_required: #energy_required,
                category: #category,
                enabled: #enabled,
                subgroup: #subgroup,
                order: #order,
                ..Default::default()
            }
        });
    }

    TokenStream::from(quote::quote! {
        pub struct Recipes;

        impl Recipes {
            #( #const_defs )*
        }

        pub fn register_recipes() {
            data.extend([
                #( #extend_items, )*
            ]);
        }
    })
}

fn option_f64_tokens(value: Option<f64>) -> TokenStream2 {
    value.map_or_else(|| quote::quote! { None }, |v| quote::quote! { Some(#v) })
}

fn option_bool_tokens(value: Option<bool>) -> TokenStream2 {
    value.map_or_else(|| quote::quote! { None }, |v| quote::quote! { Some(#v) })
}

struct RecipesMacroInput {
    recipes: Vec<RecipeProtoEntry>,
}

struct RecipeProtoEntry {
    ident: syn::Ident,
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
    name: String,
    amount: i64,
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
        let ident: syn::Ident = input.parse()?;
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
            let field: syn::Ident = content.parse()?;
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
                    let lit: syn::LitBool = content.parse()?;
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
    if input.peek(syn::LitFloat) {
        let lit: syn::LitFloat = input.parse()?;
        lit.base10_parse()
    } else {
        let lit: syn::LitInt = input.parse()?;
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

        let mut name: Option<String> = None;
        let mut amount: Option<i64> = None;

        while !content.is_empty() {
            let field: syn::Ident = content.parse()?;
            let _: Token![=] = content.parse()?;
            match field.to_string().as_str() {
                "name" => {
                    let lit: LitStr = content.parse()?;
                    name = Some(lit.value());
                }
                "amount" => {
                    let lit: syn::LitInt = content.parse()?;
                    amount = Some(lit.base10_parse()?);
                }
                other => {
                    return Err(syn::Error::new(
                        field.span(),
                        format!(
                            "unknown recipe component field `{other}`; expected `name` or `amount`"
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

/// Declare Factorio locale entries in Rust.
///
/// Keys that reference associated constants (e.g. `Settings::CASUAL_MODE`) are
/// type-checked by rustc. The frontend resolves them to the constant's string
/// value when assembling `locale/<lang>/*.cfg`.
///
/// # Example
/// ```ignore
/// factorio_rs::locale! {
///     file = "settings",
///
///     en {
///         mod_setting_name {
///             Settings::CASUAL_MODE = "Casual mode",
///         }
///         mod_setting_description {
///             Settings::CASUAL_MODE = "Relax some rules.",
///         }
///         "my-mod-messages" {
///             "hello" = "Hello engineer!",
///         }
///     }
/// }
/// ```
#[proc_macro]
pub fn locale(input: TokenStream) -> TokenStream {
    let LocaleInput { languages, .. } = parse_macro_input!(input as LocaleInput);

    let mut checks = Vec::<TokenStream2>::new();
    for lang in &languages {
        for category in &lang.categories {
            for entry in &category.entries {
                if let LocaleKey::Path(path) = &entry.key {
                    checks.push(quote::quote! {
                        let _: &'static str = #path;
                    });
                }
                let value = &entry.value;
                let value_text = value.value();
                if value_text.contains('\n') || value_text.contains('\r') {
                    return syn::Error::new_spanned(value, "locale values must be a single line")
                        .to_compile_error()
                        .into();
                }
            }
        }
    }

    TokenStream::from(quote::quote! {
        const _: () = {
            #( #checks )*
        };
    })
}

struct LocaleInput {
    #[allow(dead_code)]
    file: Option<String>,
    languages: Vec<LocaleLanguageBlock>,
}

struct LocaleLanguageBlock {
    #[allow(dead_code)]
    lang: String,
    categories: Vec<LocaleCategoryBlock>,
}

struct LocaleCategoryBlock {
    #[allow(dead_code)]
    name: String,
    entries: Vec<LocaleEntry>,
}

struct LocaleEntry {
    key: LocaleKey,
    value: LitStr,
}

enum LocaleKey {
    Path(Path),
    #[allow(dead_code)]
    Literal(String),
}

impl Parse for LocaleInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut file = None;
        if input.peek(syn::Ident) {
            let fork = input.fork();
            let kw: syn::Ident = fork.parse()?;
            if kw == "file" && fork.peek(Token![=]) {
                let _: syn::Ident = input.parse()?;
                let _: Token![=] = input.parse()?;
                let lit: LitStr = input.parse()?;
                file = Some(lit.value());
                let _: Option<Token![,]> = input.parse()?;
            }
        }

        let mut languages = Vec::new();
        while !input.is_empty() {
            languages.push(input.parse()?);
        }

        if languages.is_empty() {
            return Err(input.error("expected at least one language block such as `en { ... }`"));
        }

        Ok(Self { file, languages })
    }
}

impl Parse for LocaleLanguageBlock {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let lang = if input.peek(LitStr) {
            input.parse::<LitStr>()?.value()
        } else {
            input.parse::<syn::Ident>()?.to_string()
        };

        let content;
        syn::braced!(content in input);

        let mut categories = Vec::new();
        while !content.is_empty() {
            categories.push(content.parse()?);
        }

        Ok(Self { lang, categories })
    }
}

impl Parse for LocaleCategoryBlock {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = if input.peek(LitStr) {
            input.parse::<LitStr>()?.value()
        } else {
            let ident: syn::Ident = input.parse()?;
            ident.to_string().replace('_', "-")
        };

        let content;
        syn::braced!(content in input);

        let mut entries = Vec::new();
        while !content.is_empty() {
            entries.push(content.parse()?);
        }

        Ok(Self { name, entries })
    }
}

impl Parse for LocaleEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key = if input.peek(LitStr) {
            LocaleKey::Literal(input.parse::<LitStr>()?.value())
        } else {
            LocaleKey::Path(input.parse()?)
        };
        let _: Token![=] = input.parse()?;
        let value: LitStr = input.parse()?;
        let _: Option<Token![,]> = input.parse()?;
        Ok(Self { key, value })
    }
}
