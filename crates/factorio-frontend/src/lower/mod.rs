use std::collections::BTreeMap;

use factorio_ir::lint::{Diagnostic, LintConfig};

use syn::{ImplItem, Item, Visibility};

use crate::{
    error::{FrontendError, FrontendResult},
    paths::require_local_name,
};

mod assembling_machines;
pub mod assert_macros;
pub mod attrs;
pub mod context;
pub mod event_filter;
pub mod event_handler;
pub mod expressions;
mod extra_protos;
mod fluids;
pub mod functions;
pub mod imports;
mod items;
pub mod iterators;
mod locale;
pub use locale::{parse_pending as parse_locale_pending, resolve_project_locales};
pub mod metadata;
mod mod_settings;
pub mod print;
mod proto_macros;
mod recipes;
mod serde_json;
pub mod statements;
pub mod structs;
mod technologies;
mod test_steps;
pub mod tests;
mod traits;
pub use traits::{TraitCatalog, build_trait_catalog};
pub mod types;
pub mod util;

use context::LowerContext;
use expressions::lower_expression;
use functions::{lower_function, lower_impl_method};
use imports::{lower_use, merge_imports};
use metadata::{extract_doc_comments, struct_header_comment};
use structs::{PendingEnum, PendingStruct, impl_type_name, lower_struct_fields};
use util::{item_name, item_name_impl, location};

/// Options for lowering a Rust module into IR.
#[derive(Debug, Clone)]
pub struct ParseOptions<'a> {
    pub module_prefix: &'a str,
    pub lints: &'a LintConfig,
    pub bindings: Option<&'a crate::BindingRegistry>,
    /// Cargo `[package].name` / Factorio mod id (for `item!` icon path rewriting).
    pub mod_name: Option<&'a str>,
    /// Project-wide trait catalog for same-crate cross-module `use` of traits.
    pub trait_catalog: Option<&'a traits::TraitCatalog>,
}

impl<'a> ParseOptions<'a> {
    #[must_use]
    pub const fn new(lints: &'a LintConfig) -> Self {
        Self {
            module_prefix: "",
            lints,
            bindings: None,
            mod_name: None,
            trait_catalog: None,
        }
    }

    #[must_use]
    pub const fn with_prefix(mut self, module_prefix: &'a str) -> Self {
        self.module_prefix = module_prefix;
        self
    }

    #[must_use]
    pub const fn with_bindings(mut self, bindings: &'a crate::BindingRegistry) -> Self {
        self.bindings = Some(bindings);
        self
    }

    #[must_use]
    pub const fn with_mod_name(mut self, mod_name: &'a str) -> Self {
        self.mod_name = Some(mod_name);
        self
    }

    #[must_use]
    pub const fn with_trait_catalog(mut self, catalog: &'a traits::TraitCatalog) -> Self {
        self.trait_catalog = Some(catalog);
        self
    }

    #[must_use]
    pub fn bindings(&self) -> &crate::BindingRegistry {
        if let Some(bindings) = self.bindings {
            return bindings;
        }
        empty_binding_registry()
    }
}

fn empty_binding_registry() -> &'static crate::BindingRegistry {
    static EMPTY: std::sync::OnceLock<crate::BindingRegistry> = std::sync::OnceLock::new();
    EMPTY.get_or_init(crate::BindingRegistry::new)
}

/// Parse Rust source into a [`factorio_ir::module::Module`].
///
/// Uses [`LintConfig::allow_all`] so unit tests and ad-hoc parsing are not blocked.
/// The CLI applies `Factorio.toml` `[lints]` (default **deny**) via
/// [`parse_module_with_options`].
///
/// # Errors
/// Returns `Err` if the Rust source fails to parse
pub fn parse_module(
    source: &str,
    module_name: &str,
) -> FrontendResult<factorio_ir::module::Module> {
    let lints = LintConfig::allow_all();
    let mut diagnostics = Vec::new();
    let module = parse_module_with_options(
        source,
        module_name,
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )?;
    let _ = diagnostics;
    Ok(module)
}

/// Like [`parse_module`] but applies `module_prefix` to all generated module
/// local names (e.g. `"ms"` turns `settings` into `ms_settings`).
///
/// # Errors
/// Returns `Err` if the Rust source fails to parse
pub fn parse_module_with_prefix(
    source: &str,
    module_name: &str,
    module_prefix: &str,
) -> FrontendResult<factorio_ir::module::Module> {
    let lints = LintConfig::allow_all();
    let mut diagnostics = Vec::new();
    let module = parse_module_with_options(
        source,
        module_name,
        &ParseOptions::new(&lints).with_prefix(module_prefix),
        &mut diagnostics,
    )?;
    let _ = diagnostics;
    Ok(module)
}

/// Parse with explicit lint configuration; appends warn/deny diagnostics.
///
/// # Errors
/// Returns `Err` on hard lowering failures (unsupported syntax, etc.). Lint
/// denies are collected into `diagnostics` instead of failing immediately.
pub fn parse_module_with_options(
    source: &str,
    module_name: &str,
    options: &ParseOptions<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> FrontendResult<factorio_ir::module::Module> {
    let file = syn::parse_file(source)?;
    let stage = factorio_ir::stage::Stage::from_module_name(module_name).ok_or_else(|| {
        FrontendError::InvalidModuleStage {
            module: module_name.to_string(),
        }
    })?;
    lower_items(
        &file.items,
        module_name,
        stage,
        options.module_prefix,
        options.bindings(),
        options.lints,
        diagnostics,
        None,
        options.mod_name,
        options.trait_catalog,
    )
    .and_then(|mut module| {
        locale::resolve_project_locales(std::slice::from_mut(&mut module))?;
        Ok(module)
    })
}

/// Lower a discovered module into IR.
///
/// # Errors
/// Returns `Err` if lowering fails.
pub fn parse_discovered_module(
    discovered: &crate::discovery::DiscoveredModule,
) -> FrontendResult<factorio_ir::module::Module> {
    parse_discovered_module_with_prefix(discovered, "")
}

/// Like [`parse_discovered_module`] but applies `module_prefix` to all
/// generated module local names.
///
/// # Errors
/// Returns `Err` if lowering fails.
pub fn parse_discovered_module_with_prefix(
    discovered: &crate::discovery::DiscoveredModule,
    module_prefix: &str,
) -> FrontendResult<factorio_ir::module::Module> {
    let lints = LintConfig::allow_all();
    let mut diagnostics = Vec::new();
    parse_discovered_module_with_options(
        discovered,
        &ParseOptions::new(&lints).with_prefix(module_prefix),
        &mut diagnostics,
    )
}

/// Lower a discovered module with lint configuration.
///
/// # Errors
/// Returns `Err` on hard lowering failures. Lint denies are collected into
/// `diagnostics` instead of failing immediately.
pub fn parse_discovered_module_with_options(
    discovered: &crate::discovery::DiscoveredModule,
    options: &ParseOptions<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> FrontendResult<factorio_ir::module::Module> {
    lower_items(
        &discovered.items,
        &discovered.module_name,
        discovered.stage,
        options.module_prefix,
        options.bindings(),
        options.lints,
        diagnostics,
        discovered.default_export.as_ref(),
        options.mod_name,
        options.trait_catalog,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn lower_items(
    items: &[Item],
    module_name: &str,
    stage: factorio_ir::stage::Stage,
    module_prefix: &str,
    bindings: &crate::BindingRegistry,
    lints: &LintConfig,
    diagnostics: &mut Vec<Diagnostic>,
    default_export: Option<&factorio_ir::function::ExportMeta>,
    mod_name: Option<&str>,
    trait_catalog: Option<&traits::TraitCatalog>,
) -> FrontendResult<factorio_ir::module::Module> {
    let mut body = Vec::new();
    let mut symbols = Vec::new();
    let mut use_imports = Vec::new();
    let mut inline_imports = Vec::new();
    let mut submodules = Vec::new();
    let mut structs = BTreeMap::<String, PendingStruct>::new();
    let mut enums = BTreeMap::<String, PendingEnum>::new();
    let mut pending_locales = Vec::new();
    let mut ctx = LowerContext {
        imports: &mut inline_imports,
        module_prefix,
        bindings,
        bare_import_renames: std::collections::HashMap::new(),
        remote_locals: std::collections::HashMap::new(),
        remote_fn_locals: std::collections::HashMap::new(),
        binding_types: std::collections::HashMap::new(),
        enums: std::collections::HashMap::new(),
        type_aliases: std::collections::HashMap::new(),
        option_bindings: std::collections::HashSet::new(),
        traits: std::collections::BTreeMap::new(),
        user_structs: std::collections::HashSet::new(),
        dyn_locals: std::collections::HashMap::new(),
        dyn_fn_params: std::collections::HashMap::new(),
        assoc_bindings: std::collections::HashMap::new(),
        vtables: Vec::new(),
        lints,
        diagnostics,
        try_hoists: Vec::new(),
        try_tmp_counter: 0,
    };
    types::collect_type_aliases(items, &mut ctx.type_aliases)?;
    traits::collect_traits(items, &mut ctx.traits)?;
    if let Some(catalog) = trait_catalog {
        traits::seed_traits_from_imports(items, catalog, &mut ctx.traits, module_prefix, bindings)?;
    }
    collect_user_structs(items, &mut ctx.user_structs);
    traits::collect_dyn_fn_params(items, &mut ctx.dyn_fn_params);
    let mut module_state = ModuleLowerState {
        module_name,
        stage,
        body: &mut body,
        symbols: &mut symbols,
        use_imports: &mut use_imports,
        submodules: &mut submodules,
        structs: &mut structs,
        enums: &mut enums,
        pending_locales: &mut pending_locales,
        default_export: default_export.cloned(),
        mod_name,
    };

    for item in items {
        lower_top_level_item(item, module_name, &mut module_state, &mut ctx)?;
    }

    finalize_pending_structs(structs, &mut body, &mut symbols);
    finalize_pending_enums(enums, &mut body, &mut symbols);

    let vtables = std::mem::take(&mut ctx.vtables);
    drop(ctx);

    let mut all_imports = use_imports;
    all_imports.extend(inline_imports);

    Ok(factorio_ir::module::Module {
        name: module_name.to_string(),
        stage,
        body: factorio_ir::block::Block { statements: body },
        symbols,
        imports: merge_imports(all_imports, module_prefix),
        submodules,
        locales: Vec::new(),
        pending_locales,
        vtables,
    })
}

struct ModuleLowerState<'a> {
    module_name: &'a str,
    stage: factorio_ir::stage::Stage,
    body: &'a mut Vec<factorio_ir::statement::Statement>,
    symbols: &'a mut Vec<factorio_ir::module::Symbol>,
    use_imports: &'a mut Vec<imports::ImportFragment>,
    submodules: &'a mut Vec<String>,
    structs: &'a mut BTreeMap<String, PendingStruct>,
    enums: &'a mut BTreeMap<String, PendingEnum>,
    pending_locales: &'a mut Vec<factorio_ir::locale::PendingLocaleFile>,
    default_export: Option<factorio_ir::function::ExportMeta>,
    mod_name: Option<&'a str>,
}

pub fn collect_user_structs(items: &[syn::Item], out: &mut std::collections::HashSet<String>) {
    for item in items {
        match item {
            syn::Item::Struct(s) => {
                out.insert(s.ident.to_string());
            }
            syn::Item::Mod(m) if m.content.is_some() => {
                if let Some((_, nested)) = &m.content {
                    collect_user_structs(nested, out);
                }
            }
            _ => {}
        }
    }
}

fn lower_top_level_item(
    item: &Item,
    module_name: &str,
    module_state: &mut ModuleLowerState<'_>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    match item {
        Item::Fn(function) => lower_top_level_fn(function, module_state, ctx)?,
        Item::Const(item_const) => lower_top_level_const(item_const, module_state, ctx)?,
        Item::Struct(item_struct) => {
            lower_struct_item(item_struct, module_state.structs, module_state.enums, ctx)?;
        }
        Item::Enum(item_enum) => {
            lower_enum_item(item_enum, module_state.structs, module_state.enums, ctx)?;
        }
        Item::Impl(item_impl) => {
            lower_impl_item(item_impl, module_state.structs, module_state.enums, ctx)?;
        }
        Item::Trait(_) | Item::Type(_) => {
            // Catalogued before lowering (`collect_traits` / `collect_type_aliases`); no IR.
        }
        Item::Use(use_item) => lower_top_level_use(use_item, module_state, ctx)?,
        Item::Mod(item_mod) if item_mod.content.is_none() => {
            lower_file_mod(item_mod, module_name, module_state);
        }
        Item::Mod(item_mod) => {
            lower_inline_mod(item_mod, module_name, module_state, ctx)?;
        }
        Item::Macro(mac) => {
            lower_known_macro(mac, module_name, module_state, ctx)?;
        }
        item => {
            return Err(FrontendError::UnsupportedItem {
                item: item_name(item),
                location: location(item),
            });
        }
    }

    Ok(())
}

fn lower_top_level_fn(
    function: &syn::ItemFn,
    module_state: &mut ModuleLowerState<'_>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    let mut lowered = lower_function(function, ctx)?;
    apply_default_export(
        &mut lowered,
        &function.vis,
        module_state.default_export.as_ref(),
    );
    let statement = factorio_ir::statement::Statement::FunctionDecl(lowered);
    if let factorio_ir::statement::Statement::FunctionDecl(ref func) = statement
        && func.event.is_some()
        && module_state.stage != factorio_ir::stage::Stage::Control
    {
        return Err(FrontendError::EventOutsideControlStage {
            module: module_state.module_name.to_string(),
        });
    }
    push_scoped_statement(
        statement,
        &function.vis,
        module_state.body,
        module_state.symbols,
    );
    Ok(())
}

fn lower_top_level_const(
    item_const: &syn::ItemConst,
    module_state: &mut ModuleLowerState<'_>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    let value = lower_expression(&item_const.expr, ctx, None)?;
    let name = item_const.ident.to_string();
    push_scoped_statement(
        factorio_ir::statement::Statement::VariableDecl {
            name,
            ty: factorio_ir::r#type::Type::Void,
            source_type: None,
            value,
        },
        &item_const.vis,
        module_state.body,
        module_state.symbols,
    );
    Ok(())
}

fn lower_top_level_use(
    use_item: &syn::ItemUse,
    module_state: &mut ModuleLowerState<'_>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    let fragments = lower_use(
        use_item,
        ctx.module_prefix,
        ctx.bindings,
        &mut ctx.remote_locals,
        &mut ctx.remote_fn_locals,
    )?;
    // Populate the bare-import rename map so that path expressions like
    // `adjacent_blacklist::check` get rewritten to `ms_adjacent_blacklist.check`.
    // We only do this for bare module imports (item == None), NOT for item
    // imports like `use crate::settings::Settings` - that keeps the Factorio
    // global `settings` safe from being renamed.
    if !ctx.module_prefix.is_empty() {
        for fragment in &fragments {
            if fragment.item.is_none() {
                let bare = require_local_name(&fragment.module);
                if bare != fragment.require_local {
                    ctx.bare_import_renames
                        .insert(bare, fragment.require_local.clone());
                }
            }
        }
    }
    module_state.use_imports.extend(fragments);
    Ok(())
}

fn lower_file_mod(
    item_mod: &syn::ItemMod,
    module_name: &str,
    module_state: &mut ModuleLowerState<'_>,
) {
    // File modules gated on `#[cfg(test)]` are only loaded by the test runner.
    if attrs::is_cfg_test(&item_mod.attrs) {
        return;
    }
    module_state
        .submodules
        .push(submodule_path(module_name, &item_mod.ident.to_string()));
}

fn lower_inline_mod(
    item_mod: &syn::ItemMod,
    module_name: &str,
    module_state: &mut ModuleLowerState<'_>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    // `#[cfg(test)]` modules are only lowered by the test runner.
    if attrs::is_cfg_test(&item_mod.attrs) {
        return Ok(());
    }
    let Some(export) = item_mod
        .attrs
        .iter()
        .find_map(attrs::parse_factorio_export_attribute)
    else {
        ctx.emit_lint(
            factorio_ir::lint::LintId::SkippedMod,
            format!(
                "inline `mod {}` is skipped when lowering; add `#[factorio_rs::export]` or use a file module",
                item_mod.ident
            ),
            util::location(item_mod),
        )?;
        return Ok(());
    };
    let Some((_, items)) = &item_mod.content else {
        return Ok(());
    };
    let previous = module_state.default_export.take();
    module_state.default_export = Some(export);
    for nested in items {
        lower_top_level_item(nested, module_name, module_state, ctx)?;
    }
    module_state.default_export = previous;
    Ok(())
}

fn lower_expanded_items(
    items: &[Item],
    module_name: &str,
    module_state: &mut ModuleLowerState<'_>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    for item in items {
        lower_top_level_item(item, module_name, module_state, ctx)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn lower_known_macro(
    mac: &syn::ItemMacro,
    module_name: &str,
    module_state: &mut ModuleLowerState<'_>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    let tokens = mac.mac.tokens.clone();
    let mod_name = module_state.mod_name;
    let expanded = if is_known_macro(&mac.mac, "mod_settings") {
        Some(mod_settings::expand(tokens)?)
    } else if is_known_macro(&mac.mac, "item") {
        Some(items::expand(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "recipe") {
        Some(recipes::expand(tokens)?)
    } else if is_known_macro(&mac.mac, "technology") {
        Some(technologies::expand(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "fluid") {
        Some(fluids::expand(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "assembling_machine") {
        Some(assembling_machines::expand(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "container") {
        Some(extra_protos::expand_container(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "inserter") {
        Some(extra_protos::expand_inserter(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "transport_belt") {
        Some(extra_protos::expand_transport_belt(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "furnace") {
        Some(extra_protos::expand_furnace(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "mining_drill") {
        Some(extra_protos::expand_mining_drill(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "lab") {
        Some(extra_protos::expand_lab(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "resource") {
        Some(extra_protos::expand_resource(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "tile") {
        Some(extra_protos::expand_tile(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "autoplace_control") {
        Some(extra_protos::expand_autoplace_control(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "recipe_category") {
        Some(extra_protos::expand_recipe_category(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "item_group") {
        Some(extra_protos::expand_item_group(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "item_subgroup") {
        Some(extra_protos::expand_item_subgroup(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "module") {
        Some(extra_protos::expand_module(tokens, mod_name)?)
    } else if is_known_macro(&mac.mac, "locale") {
        module_state
            .pending_locales
            .extend(locale::parse_pending(tokens)?);
        None
    } else {
        return Err(FrontendError::UnsupportedItem {
            item: "macro".to_string(),
            location: location(mac),
        });
    };

    if let Some(expanded) = expanded {
        lower_expanded_items(&expanded, module_name, module_state, ctx)?;
    }
    Ok(())
}

fn lower_struct_item(
    item_struct: &syn::ItemStruct,
    structs: &mut BTreeMap<String, PendingStruct>,
    enums: &BTreeMap<String, PendingEnum>,
    ctx: &LowerContext<'_>,
) -> FrontendResult<()> {
    let name = item_struct.ident.to_string();
    if enums.contains_key(&name) {
        return Err(FrontendError::UnsupportedItem {
            item: format!("struct `{name}` collides with an enum"),
            location: location(item_struct),
        });
    }
    let entry = structs
        .entry(name)
        .or_insert_with(|| PendingStruct::new(item_struct.vis.clone()));
    entry.visibility = item_struct.vis.clone();
    entry.fields =
        lower_struct_fields(&item_struct.fields, &ctx.type_aliases, &ctx.assoc_bindings)?;
    entry.doc = extract_doc_comments(&item_struct.attrs);
    Ok(())
}

fn lower_impl_item(
    item_impl: &syn::ItemImpl,
    structs: &mut BTreeMap<String, PendingStruct>,
    enums: &mut BTreeMap<String, PendingEnum>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    let trait_name = if let Some((_, trait_path, _)) = &item_impl.trait_ {
        Some(trait_impl_name(trait_path, item_impl)?)
    } else {
        None
    };

    let struct_name = impl_type_name(&item_impl.self_ty)?;
    if let Some(entry) = enums.get_mut(&struct_name) {
        if trait_name.is_some() {
            return Err(FrontendError::UnsupportedItem {
                item: format!("trait impl for enum `{struct_name}`"),
                location: location(item_impl),
            });
        }
        for impl_item in &item_impl.items {
            match impl_item {
                ImplItem::Fn(method) => {
                    entry
                        .methods
                        .push(lower_impl_method(method, &struct_name, ctx)?);
                }
                ImplItem::Const(item) => entry.constants.push((
                    item.ident.to_string(),
                    lower_expression(&item.expr, ctx, Some(&struct_name))?,
                )),
                item => {
                    return Err(FrontendError::UnsupportedItem {
                        item: item_name_impl(item),
                        location: location(item),
                    });
                }
            }
        }
        return Ok(());
    }

    let entry = structs
        .entry(struct_name.clone())
        .or_insert_with(|| PendingStruct::new(Visibility::Inherited));

    if let Some(trait_name) = trait_name.as_ref() {
        return traits::lower_trait_impl(item_impl, trait_name, &struct_name, entry, ctx);
    }

    for impl_item in &item_impl.items {
        match impl_item {
            ImplItem::Fn(method) => {
                let method_name = method.sig.ident.to_string();
                if entry.methods.iter().any(|m| m.name == method_name) {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!("method `{method_name}` already defined on `{struct_name}`"),
                        location: location(method),
                    });
                }
                entry
                    .methods
                    .push(lower_impl_method(method, &struct_name, ctx)?);
            }
            ImplItem::Const(item) => entry.constants.push((
                item.ident.to_string(),
                lower_expression(&item.expr, ctx, Some(&struct_name))?,
            )),
            item => {
                return Err(FrontendError::UnsupportedItem {
                    item: item_name_impl(item),
                    location: location(item),
                });
            }
        }
    }

    Ok(())
}

fn trait_impl_name(trait_path: &syn::Path, item_impl: &syn::ItemImpl) -> FrontendResult<String> {
    if trait_path.segments.len() != 1 {
        return Err(FrontendError::UnsupportedItem {
            item: "trait path with multiple segments".to_string(),
            location: location(item_impl),
        });
    }
    Ok(trait_path.segments[0].ident.to_string())
}

fn lower_enum_item(
    item_enum: &syn::ItemEnum,
    structs: &BTreeMap<String, PendingStruct>,
    enums: &mut BTreeMap<String, PendingEnum>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    if item_enum
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("repr"))
    {
        return Err(FrontendError::UnsupportedItem {
            item: "repr enum".to_string(),
            location: location(item_enum),
        });
    }
    let name = item_enum.ident.to_string();
    let mut variants = Vec::with_capacity(item_enum.variants.len());
    let mut infos = Vec::with_capacity(item_enum.variants.len());
    for variant in &item_enum.variants {
        if variant.discriminant.is_some() {
            return Err(FrontendError::UnsupportedItem {
                item: "enum discriminant".to_string(),
                location: location(variant),
            });
        }
        let fields = match &variant.fields {
            syn::Fields::Unit => factorio_ir::enumeration::EnumVariantFields::Unit,
            syn::Fields::Unnamed(fields) => factorio_ir::enumeration::EnumVariantFields::Tuple {
                types: fields
                    .unnamed
                    .iter()
                    .map(|field| {
                        types::lower_type(&field.ty, &ctx.type_aliases, &ctx.assoc_bindings)
                    })
                    .collect::<FrontendResult<Vec<_>>>()?,
            },
            syn::Fields::Named(fields) => {
                factorio_ir::enumeration::EnumVariantFields::Named(lower_struct_fields(
                    &syn::Fields::Named(fields.clone()),
                    &ctx.type_aliases,
                    &ctx.assoc_bindings,
                )?)
            }
        };
        let info_fields = match &fields {
            factorio_ir::enumeration::EnumVariantFields::Unit => context::EnumVariantFields::Unit,
            factorio_ir::enumeration::EnumVariantFields::Tuple { types } => {
                context::EnumVariantFields::Tuple(types.len())
            }
            factorio_ir::enumeration::EnumVariantFields::Named(_) => {
                context::EnumVariantFields::Named
            }
        };
        let variant_name = variant.ident.to_string();
        infos.push(context::EnumVariantInfo {
            name: variant_name.clone(),
            fields: info_fields,
        });
        variants.push(factorio_ir::enumeration::EnumVariant {
            name: variant_name,
            fields,
        });
    }
    if structs.contains_key(&name) {
        return Err(FrontendError::UnsupportedItem {
            item: format!("enum `{name}` collides with a struct"),
            location: location(item_enum),
        });
    }
    ctx.enums.insert(name.clone(), infos);
    let entry = enums
        .entry(name)
        .or_insert_with(|| PendingEnum::new(item_enum.vis.clone()));
    entry.visibility = item_enum.vis.clone();
    entry.variants = variants;
    entry.doc = extract_doc_comments(&item_enum.attrs);
    Ok(())
}

fn finalize_pending_enums(
    enums: BTreeMap<String, PendingEnum>,
    body: &mut Vec<factorio_ir::statement::Statement>,
    symbols: &mut Vec<factorio_ir::module::Symbol>,
) {
    for (name, pending) in enums {
        let lowered = factorio_ir::statement::Statement::EnumDecl(factorio_ir::enumeration::Enum {
            name,
            variants: pending.variants,
            constants: pending.constants,
            methods: pending.methods,
            doc: pending.doc,
            debug: None,
        });
        push_scoped_statement(lowered, &pending.visibility, body, symbols);
    }
}

fn finalize_pending_structs(
    structs: BTreeMap<String, PendingStruct>,
    body: &mut Vec<factorio_ir::statement::Statement>,
    symbols: &mut Vec<factorio_ir::module::Symbol>,
) {
    for (name, pending_struct) in structs {
        let lowered =
            factorio_ir::statement::Statement::StructDecl(factorio_ir::structure::Struct {
                name: name.clone(),
                fields: pending_struct.fields.clone(),
                constants: pending_struct.constants,
                methods: pending_struct.methods,
                doc: pending_struct.doc,
                debug: Some(factorio_ir::debug::StructDebug {
                    header_comment: struct_header_comment(
                        &pending_struct.visibility,
                        &name,
                        &pending_struct.fields,
                    ),
                }),
            });

        push_scoped_statement(lowered, &pending_struct.visibility, body, symbols);
    }
}

fn push_scoped_statement(
    statement: factorio_ir::statement::Statement,
    visibility: &Visibility,
    body: &mut Vec<factorio_ir::statement::Statement>,
    symbols: &mut Vec<factorio_ir::module::Symbol>,
) {
    match visibility {
        Visibility::Public(_) => symbols.push(factorio_ir::module::Symbol {
            scope: factorio_ir::scope::Scope::Public,
            statement,
        }),
        _ => body.push(statement),
    }
}

/// Inherit module-level `#[factorio_rs::export]` onto public functions that lack
/// their own export attribute.
fn apply_default_export(
    function: &mut factorio_ir::function::Function,
    visibility: &Visibility,
    default_export: Option<&factorio_ir::function::ExportMeta>,
) {
    if function.export.is_some() {
        return;
    }
    if !matches!(visibility, Visibility::Public(_)) {
        return;
    }
    if let Some(export) = default_export {
        function.export = Some(export.clone());
    }
}

fn submodule_path(module_name: &str, child: &str) -> String {
    format!("{module_name}.{child}")
}

/// Returns `true` if the macro's path ends with `name` (e.g. `mod_settings`).
/// Matches both `mod_settings!` and `factorio_rs::mod_settings!`.
fn is_known_macro(mac: &syn::Macro, name: &str) -> bool {
    mac.path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == name)
}
