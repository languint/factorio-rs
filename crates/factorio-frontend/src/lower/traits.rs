use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use syn::{FnArg, Item, ItemTrait, ReturnType, TraitItem, Type, TypeParamBound};

use crate::error::{FrontendError, FrontendResult};

use super::{
    context::{DynLocal, TraitInfo, TraitMethodInfo},
    imports::lower_use,
    util::location,
};

/// Project-wide trait definitions keyed by module path then trait name.
#[derive(Clone, Default)]
pub struct TraitCatalog {
    /// `module_name` (e.g. `"shared.alert"`) -> trait name -> [`TraitInfo`]
    by_module: BTreeMap<String, BTreeMap<String, TraitInfo>>,
}

impl std::fmt::Debug for TraitCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TraitCatalog")
            .field("modules", &self.by_module.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl TraitCatalog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, module: &str, info: TraitInfo) {
        self.by_module
            .entry(module.to_string())
            .or_default()
            .insert(info.name.clone(), info);
    }

    #[must_use]
    pub fn get(&self, module: &str, trait_name: &str) -> Option<&TraitInfo> {
        self.by_module.get(module)?.get(trait_name)
    }

    /// Find a trait by name across modules when the name is unique.
    #[must_use]
    pub fn find_unique(&self, trait_name: &str) -> Option<&TraitInfo> {
        let mut found = None;
        for module_traits in self.by_module.values() {
            if let Some(info) = module_traits.get(trait_name) {
                if found.is_some() {
                    return None;
                }
                found = Some(info);
            }
        }
        found
    }
}

/// Build a [`TraitCatalog`] from every source file in the project.
///
/// # Errors
/// Returns `Err` if a source file fails to parse or a trait is unsupported.
pub fn build_trait_catalog(
    sources: &[(PathBuf, String)],
    source_dir: &Path,
) -> FrontendResult<TraitCatalog> {
    let mut catalog = TraitCatalog::new();
    for (path, source) in sources {
        for spec in crate::discovery::discover_modules(source_dir, path, source)? {
            collect_traits_into_catalog(&spec.items, &spec.module_name, &mut catalog)?;
        }
    }
    Ok(catalog)
}

/// Recursively catalog traits under `module_name`, descending into inline mods.
pub fn collect_traits_into_catalog(
    items: &[Item],
    module_name: &str,
    catalog: &mut TraitCatalog,
) -> FrontendResult<()> {
    for item in items {
        match item {
            Item::Trait(item_trait) => {
                catalog.insert(module_name, catalog_trait_info(item_trait)?);
            }
            Item::Mod(item_mod) => {
                if let Some((_, nested)) = &item_mod.content {
                    let child = if module_name.is_empty() {
                        item_mod.ident.to_string()
                    } else {
                        format!("{module_name}.{}", item_mod.ident)
                    };
                    collect_traits_into_catalog(nested, &child, catalog)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Pre-scan top-level (and nested export) items for trait definitions.
pub fn collect_traits(
    items: &[Item],
    traits: &mut BTreeMap<String, TraitInfo>,
) -> FrontendResult<()> {
    for item in items {
        match item {
            Item::Trait(item_trait) => {
                catalog_trait(item_trait, traits)?;
            }
            Item::Mod(item_mod) if item_mod.attrs.iter().any(matches_export) => {
                if let Some((_, nested)) = &item_mod.content {
                    collect_traits(nested, traits)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Seed `traits` from `use crate::module::Trait` imports against the project catalog.
pub fn seed_traits_from_imports(
    items: &[Item],
    catalog: &TraitCatalog,
    traits: &mut BTreeMap<String, TraitInfo>,
    module_prefix: &str,
    bindings: &crate::bindings::BindingRegistry,
) -> FrontendResult<()> {
    let mut remote_locals = HashMap::new();
    let mut remote_fn_locals = HashMap::new();
    for item in items {
        match item {
            Item::Use(use_item) => {
                let fragments = lower_use(
                    use_item,
                    module_prefix,
                    bindings,
                    &mut remote_locals,
                    &mut remote_fn_locals,
                )?;
                for fragment in fragments {
                    let Some(imported) = &fragment.item else {
                        continue;
                    };
                    let Some(info) = catalog.get(&fragment.module, &imported.name) else {
                        continue;
                    };
                    // Local definitions win over imports.
                    traits
                        .entry(imported.local.clone())
                        .or_insert_with(|| info.clone());
                }
            }
            Item::Mod(item_mod) if item_mod.attrs.iter().any(matches_export) => {
                if let Some((_, nested)) = &item_mod.content {
                    seed_traits_from_imports(nested, catalog, traits, module_prefix, bindings)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn matches_export(attr: &syn::Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|seg| seg.ident == "export")
}

/// Catalog a trait definition into `traits`. Does not emit IR.
pub fn catalog_trait(
    item_trait: &ItemTrait,
    traits: &mut BTreeMap<String, TraitInfo>,
) -> FrontendResult<()> {
    let info = catalog_trait_info(item_trait)?;
    let name = info.name.clone();
    if traits.contains_key(&name) {
        return Err(FrontendError::UnsupportedItem {
            item: format!("duplicate trait `{name}`"),
            location: location(item_trait),
        });
    }
    traits.insert(name, info);
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn catalog_trait_info(item_trait: &ItemTrait) -> FrontendResult<TraitInfo> {
    let name = item_trait.ident.to_string();
    if !item_trait.generics.params.is_empty() {
        return Err(FrontendError::UnsupportedItem {
            item: format!("trait `{name}` with generics"),
            location: location(item_trait),
        });
    }
    if item_trait.colon_token.is_some() || !item_trait.supertraits.is_empty() {
        return Err(FrontendError::UnsupportedItem {
            item: format!("trait `{name}` with supertraits"),
            location: location(item_trait),
        });
    }

    let mut methods = BTreeMap::new();
    let mut associated_types = BTreeSet::new();
    for item in &item_trait.items {
        match item {
            TraitItem::Fn(method) => {
                let method_name = method.sig.ident.to_string();
                if !method.sig.generics.params.is_empty() {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!("generic method `{method_name}` on trait `{name}`"),
                        location: location(method),
                    });
                }
                let has_receiver = method
                    .sig
                    .inputs
                    .iter()
                    .any(|input| matches!(input, FnArg::Receiver(_)));
                let returns_self = return_type_is_self(&method.sig.output);
                let default_body = method.default.clone().map(|block| syn::ImplItemFn {
                    attrs: method.attrs.clone(),
                    vis: syn::Visibility::Inherited,
                    defaultness: None,
                    sig: method.sig.clone(),
                    block,
                });
                methods.insert(
                    method_name.clone(),
                    TraitMethodInfo {
                        name: method_name,
                        has_receiver,
                        returns_self,
                        default_body,
                    },
                );
            }
            TraitItem::Type(item) => {
                let assoc_name = item.ident.to_string();
                if !item.generics.params.is_empty() {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!(
                            "associated type `{assoc_name}` with generics on trait `{name}`"
                        ),
                        location: location(item),
                    });
                }
                if item.colon_token.is_some() || !item.bounds.is_empty() {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!(
                            "associated type `{assoc_name}` with bounds on trait `{name}`"
                        ),
                        location: location(item),
                    });
                }
                if item.default.is_some() {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!(
                            "associated type `{assoc_name}` with default on trait `{name}`"
                        ),
                        location: location(item),
                    });
                }
                if !associated_types.insert(assoc_name.clone()) {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!("duplicate associated type `{assoc_name}` on trait `{name}`"),
                        location: location(item),
                    });
                }
            }
            TraitItem::Const(item) => {
                return Err(FrontendError::UnsupportedItem {
                    item: format!("associated const `{}` on trait `{name}`", item.ident),
                    location: location(item),
                });
            }
            TraitItem::Macro(_) => {
                return Err(FrontendError::UnsupportedItem {
                    item: format!("macro in trait `{name}`"),
                    location: location(item),
                });
            }
            _ => {
                return Err(FrontendError::UnsupportedItem {
                    item: format!("unsupported trait item in `{name}`"),
                    location: location(item),
                });
            }
        }
    }

    Ok(TraitInfo {
        name,
        methods,
        associated_types,
    })
}

fn return_type_is_self(output: &ReturnType) -> bool {
    match output {
        ReturnType::Default => false,
        ReturnType::Type(_, ty) => type_is_self(ty),
    }
}

fn type_is_self(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => {
            path.qself.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].ident == "Self"
                && matches!(path.path.segments[0].arguments, syn::PathArguments::None)
        }
        Type::Reference(reference) => type_is_self(&reference.elem),
        _ => false,
    }
}

/// Extract the trait name from `&dyn Trait`, `Box<dyn Trait>`, or `dyn Trait`.
pub fn dyn_trait_name(ty: &Type) -> Option<String> {
    match ty {
        Type::TraitObject(obj) => trait_object_name(obj),
        Type::Reference(reference) => dyn_trait_name(&reference.elem),
        Type::Paren(paren) => dyn_trait_name(&paren.elem),
        Type::Group(group) => dyn_trait_name(&group.elem),
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            if segment.ident != "Box" {
                return None;
            }
            let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
                return None;
            };
            for arg in &args.args {
                if let syn::GenericArgument::Type(inner) = arg {
                    return dyn_trait_name(inner);
                }
            }
            None
        }
        _ => None,
    }
}

fn trait_object_name(obj: &syn::TypeTraitObject) -> Option<String> {
    let mut trait_name = None;
    for bound in &obj.bounds {
        match bound {
            TypeParamBound::Trait(trait_bound) => {
                if trait_bound.path.segments.len() != 1 {
                    return None;
                }
                if trait_name.is_some() {
                    return None;
                }
                trait_name = Some(trait_bound.path.segments[0].ident.to_string());
            }
            TypeParamBound::Lifetime(_) => {}
            _ => return None,
        }
    }
    trait_name
}

/// Vtable symbol for a `(Trait, Concrete)` pair.
#[must_use]
pub fn vtable_name(trait_name: &str, concrete_name: &str) -> String {
    format!("__vt_{trait_name}_{concrete_name}")
}

/// Record free functions that take `&dyn Trait` / `Box<dyn Trait>` parameters.
///
/// Call sites use this map to auto-coerce concrete arguments into fat pointers
/// without an explicit `as &dyn Trait` cast.
pub fn collect_dyn_fn_params(items: &[Item], out: &mut HashMap<String, Vec<Option<String>>>) {
    for item in items {
        match item {
            Item::Fn(function) => {
                let params: Vec<Option<String>> = function
                    .sig
                    .inputs
                    .iter()
                    .map(|input| match input {
                        FnArg::Receiver(_) => None,
                        FnArg::Typed(pat_type) => dyn_trait_name(&pat_type.ty),
                    })
                    .collect();
                if params.iter().any(Option::is_some) {
                    out.insert(function.sig.ident.to_string(), params);
                }
            }
            Item::Mod(item_mod) => {
                if let Some((_, nested)) = &item_mod.content {
                    collect_dyn_fn_params(nested, out);
                }
            }
            _ => {}
        }
    }
}

/// Reject non-object-safe traits when coercing to `dyn`.
pub fn ensure_object_safe(
    trait_info: &TraitInfo,
    loc: factorio_ir::span::SourceLoc,
) -> FrontendResult<()> {
    if !trait_info.associated_types.is_empty() {
        return Err(FrontendError::UnsupportedItem {
            item: format!(
                "trait `{}` is not object-safe: associated types",
                trait_info.name
            ),
            location: loc,
        });
    }
    for method in trait_info.methods.values() {
        if !method.has_receiver {
            return Err(FrontendError::UnsupportedItem {
                item: format!(
                    "trait `{}` is not object-safe: method `{}` has no self receiver",
                    trait_info.name, method.name
                ),
                location: loc,
            });
        }
        if method.returns_self {
            return Err(FrontendError::UnsupportedItem {
                item: format!(
                    "trait `{}` is not object-safe: method `{}` returns `Self`",
                    trait_info.name, method.name
                ),
                location: loc,
            });
        }
    }
    Ok(())
}

/// Resolve the concrete type name of an expression used in a dyn cast.
pub fn resolve_concrete_type(
    expr: &syn::Expr,
    ctx: &super::context::LowerContext<'_>,
) -> Option<String> {
    match expr {
        syn::Expr::Struct(item) => item.path.segments.last().map(|seg| seg.ident.to_string()),
        syn::Expr::Path(path) if path.path.segments.len() == 1 => {
            let name = path.path.segments[0].ident.to_string();
            ctx.binding_type(&name).map(str::to_string).or(Some(name))
        }
        syn::Expr::Call(call) => {
            peel_box_new_concrete(call, ctx).or_else(|| resolve_constructor_type(&call.func))
        }
        syn::Expr::Reference(reference) => resolve_concrete_type(&reference.expr, ctx),
        syn::Expr::Paren(paren) => resolve_concrete_type(&paren.expr, ctx),
        syn::Expr::Group(group) => resolve_concrete_type(&group.expr, ctx),
        _ => None,
    }
}

fn peel_box_new_concrete(
    call: &syn::ExprCall,
    ctx: &super::context::LowerContext<'_>,
) -> Option<String> {
    if !is_box_new_call(call) {
        return None;
    }
    let arg = call.args.first()?;
    resolve_concrete_type(arg, ctx)
}

fn is_box_new_call(call: &syn::ExprCall) -> bool {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    let segments: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    segments.len() >= 2
        && segments[segments.len() - 2] == "Box"
        && segments[segments.len() - 1] == "new"
}

fn resolve_constructor_type(func: &syn::Expr) -> Option<String> {
    let syn::Expr::Path(path) = func else {
        return None;
    };
    if path.path.segments.len() >= 2 {
        return Some(
            path.path.segments[path.path.segments.len() - 2]
                .ident
                .to_string(),
        );
    }
    None
}

/// Peel `Box::new(inner)` to `inner` for dyn packing.
pub fn peel_box_new(expr: &syn::Expr) -> &syn::Expr {
    if let syn::Expr::Call(call) = expr
        && is_box_new_call(call)
        && let Some(arg) = call.args.first()
    {
        return peel_box_new(arg);
    }
    match expr {
        syn::Expr::Reference(reference) => peel_box_new(&reference.expr),
        syn::Expr::Paren(paren) => peel_box_new(&paren.expr),
        syn::Expr::Group(group) => peel_box_new(&group.expr),
        _ => expr,
    }
}

/// Build a [`DynLocal`] binding record.
#[must_use]
pub fn dyn_local(trait_name: impl Into<String>, concrete_name: impl Into<String>) -> DynLocal {
    DynLocal {
        trait_name: trait_name.into(),
        concrete_name: concrete_name.into(),
    }
}

/// Merge a trait impl onto a pending struct (provided + default methods + vtable).
#[allow(clippy::too_many_lines)]
pub fn lower_trait_impl(
    item_impl: &syn::ItemImpl,
    trait_name: &str,
    struct_name: &str,
    entry: &mut super::structs::PendingStruct,
    ctx: &mut super::context::LowerContext<'_>,
) -> FrontendResult<()> {
    use super::functions::lower_impl_method;

    let trait_info =
        ctx.traits
            .get(trait_name)
            .cloned()
            .ok_or_else(|| FrontendError::UnsupportedItem {
                item: format!(
                    "unknown trait `{trait_name}`; define it in this module or `use` it from another module"
                ),
                location: location(item_impl),
            })?;

    let mut assoc_map: HashMap<String, Type> = HashMap::new();
    let mut provided = HashSet::new();
    let mut method_items = Vec::new();

    for impl_item in &item_impl.items {
        match impl_item {
            syn::ImplItem::Type(item) => {
                let assoc_name = item.ident.to_string();
                if !item.generics.params.is_empty() {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!("associated type `{assoc_name}` with generics in trait impl"),
                        location: location(item),
                    });
                }
                if !trait_info.associated_types.contains(&assoc_name) {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!(
                            "associated type `{assoc_name}` is not a member of trait `{trait_name}`"
                        ),
                        location: location(item),
                    });
                }
                if assoc_map.contains_key(&assoc_name) {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!("duplicate associated type `{assoc_name}` in trait impl"),
                        location: location(item),
                    });
                }
                assoc_map.insert(assoc_name, item.ty.clone());
            }
            syn::ImplItem::Fn(method) => {
                let method_name = method.sig.ident.to_string();
                reject_duplicate_method(entry, &method_name, location(method))?;
                if !trait_info.methods.contains_key(&method_name) {
                    return Err(FrontendError::UnsupportedItem {
                        item: format!(
                            "method `{method_name}` is not a member of trait `{trait_name}`"
                        ),
                        location: location(method),
                    });
                }
                provided.insert(method_name);
                method_items.push(method);
            }
            syn::ImplItem::Const(item) => {
                return Err(FrontendError::UnsupportedItem {
                    item: format!("associated const in trait impl (`{}`)", item.ident),
                    location: location(item),
                });
            }
            item => {
                return Err(FrontendError::UnsupportedItem {
                    item: super::util::item_name_impl(item),
                    location: location(item),
                });
            }
        }
    }

    for assoc_name in &trait_info.associated_types {
        if !assoc_map.contains_key(assoc_name) {
            return Err(FrontendError::UnsupportedItem {
                item: format!(
                    "trait impl for `{trait_name}` on `{struct_name}` is missing associated type `{assoc_name}`"
                ),
                location: location(item_impl),
            });
        }
    }

    ctx.assoc_bindings = assoc_map;
    for method in method_items {
        entry
            .methods
            .push(lower_impl_method(method, struct_name, ctx)?);
    }

    for (method_name, method_info) in &trait_info.methods {
        if provided.contains(method_name) {
            continue;
        }
        let Some(default_fn) = &method_info.default_body else {
            ctx.assoc_bindings.clear();
            return Err(FrontendError::UnsupportedItem {
                item: format!(
                    "trait impl for `{trait_name}` on `{struct_name}` is missing method `{method_name}`"
                ),
                location: location(item_impl),
            });
        };
        reject_duplicate_method(entry, method_name, location(item_impl))?;
        entry
            .methods
            .push(lower_impl_method(default_fn, struct_name, ctx)?);
    }
    ctx.assoc_bindings.clear();

    let vt_name = vtable_name(trait_name, struct_name);
    if !ctx.vtables.iter().any(|vt| vt.name == vt_name) {
        ctx.vtables.push(factorio_ir::module::VTable {
            name: vt_name,
            concrete_type: struct_name.to_string(),
            methods: trait_info.methods.keys().cloned().collect(),
        });
    }
    Ok(())
}

fn reject_duplicate_method(
    entry: &super::structs::PendingStruct,
    method_name: &str,
    loc: factorio_ir::span::SourceLoc,
) -> FrontendResult<()> {
    if entry.methods.iter().any(|m| m.name == method_name) {
        return Err(FrontendError::UnsupportedItem {
            item: format!("method `{method_name}` already defined"),
            location: loc,
        });
    }
    Ok(())
}
