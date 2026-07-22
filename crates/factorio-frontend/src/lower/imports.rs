use std::collections::BTreeMap;

use syn::{ItemUse, UseGroup, UseName, UsePath, UseRename, UseTree};

use crate::{
    bindings::{BindingRegistry, FactorioBinding},
    error::{FrontendError, FrontendResult},
    paths::{require_local_name, split_crate_path},
};

pub struct ImportFragment {
    pub module: String,
    pub require_local: String,
    pub item: Option<factorio_ir::module::ImportedItem>,
    pub factorio_mod: Option<String>,
    pub module_root: Option<String>,
}

struct RawUseBinding {
    segments: Vec<String>,
    rename: Option<String>,
}

pub fn lower_use(
    item: &ItemUse,
    module_prefix: &str,
    bindings: &BindingRegistry,
    remote_locals: &mut std::collections::HashMap<String, String>,
    remote_fn_locals: &mut std::collections::HashMap<String, (String, String)>,
) -> FrontendResult<Vec<ImportFragment>> {
    let mut raw_bindings = Vec::new();
    collect_use_bindings(&item.tree, &mut Vec::new(), &mut raw_bindings)?;

    let mut fragments = Vec::new();
    for binding in raw_bindings {
        if let Some(fragment) = finalize_use_binding(
            binding,
            module_prefix,
            bindings,
            remote_locals,
            remote_fn_locals,
        )? {
            fragments.push(fragment);
        }
    }

    Ok(fragments)
}

fn collect_use_bindings(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    bindings: &mut Vec<RawUseBinding>,
) -> FrontendResult<()> {
    match tree {
        UseTree::Path(UsePath { ident, tree, .. }) => {
            prefix.push(ident.to_string());
            collect_use_bindings(tree, prefix, bindings)?;
            prefix.pop();
            Ok(())
        }
        UseTree::Name(UseName { ident, .. }) => {
            if ident == "self" {
                bindings.push(RawUseBinding {
                    segments: prefix.clone(),
                    rename: None,
                });
                return Ok(());
            }
            prefix.push(ident.to_string());
            bindings.push(RawUseBinding {
                segments: prefix.clone(),
                rename: None,
            });
            prefix.pop();
            Ok(())
        }
        UseTree::Rename(UseRename { ident, rename, .. }) => {
            prefix.push(ident.to_string());
            bindings.push(RawUseBinding {
                segments: prefix.clone(),
                rename: Some(rename.to_string()),
            });
            prefix.pop();
            Ok(())
        }
        UseTree::Glob(_) => {
            // `use crate::foo::*` -> record a module-level import of `crate::foo`
            // so the module gets `require`d in Lua.
            // External globs like `use factorio_rs::prelude::*` are filtered out
            // later by `finalize_use_binding` unless they match a binding crate.
            bindings.push(RawUseBinding {
                segments: prefix.clone(),
                rename: None,
            });
            Ok(())
        }
        UseTree::Group(UseGroup { items, .. }) => {
            for item in items {
                collect_use_bindings(item, prefix, bindings)?;
            }
            Ok(())
        }
    }
}

fn finalize_use_binding(
    binding: RawUseBinding,
    module_prefix: &str,
    bindings: &BindingRegistry,
    remote_locals: &mut std::collections::HashMap<String, String>,
    remote_fn_locals: &mut std::collections::HashMap<String, (String, String)>,
) -> FrontendResult<Option<ImportFragment>> {
    let Some(first) = binding.segments.first().map(String::as_str) else {
        return Ok(None);
    };

    if first == "crate" {
        return finalize_own_mod_binding(binding, module_prefix);
    }

    if let Some(factorio_binding) = bindings.get(first) {
        return finalize_foreign_binding(
            binding,
            module_prefix,
            factorio_binding,
            remote_locals,
            remote_fn_locals,
        );
    }

    Ok(None)
}

fn finalize_own_mod_binding(
    binding: RawUseBinding,
    module_prefix: &str,
) -> FrontendResult<Option<ImportFragment>> {
    #[allow(clippy::indexing_slicing)]
    let (module_path, item_segments) = split_crate_path(&binding.segments[1..]);
    if module_path.is_empty() {
        return Err(FrontendError::UnsupportedItem {
            item: format!("use {}", binding.segments.join("::")),
            location: factorio_ir::span::SourceLoc::default().with_note("use"),
        });
    }

    Ok(Some(fragment_from_parts(
        module_path,
        &item_segments,
        binding.rename,
        module_prefix,
        None,
        None,
    )?))
}

fn finalize_foreign_binding(
    binding: RawUseBinding,
    module_prefix: &str,
    factorio_binding: &FactorioBinding,
    remote_locals: &mut std::collections::HashMap<String, String>,
    remote_fn_locals: &mut std::collections::HashMap<String, (String, String)>,
) -> FrontendResult<Option<ImportFragment>> {
    #[allow(clippy::indexing_slicing)]
    let rest = &binding.segments[1..];

    // `use provider_api::greet` - crate-root remote fn (listed in `remote_fns`).
    if rest.len() == 1
        && let Some(interface) = factorio_binding.interface.as_ref()
        && factorio_binding.remote_fns.contains(&rest[0])
    {
        let fn_name = rest[0].clone();
        let local = binding.rename.unwrap_or_else(|| fn_name.clone());
        remote_fn_locals.insert(local, (interface.clone(), fn_name));
        return Ok(None);
    }

    // `use provider_api::remote::greet` - item import from the compat shim.
    if rest.len() == 2
        && rest[0] == "remote"
        && let Some(interface) = factorio_binding.interface.as_ref()
    {
        let fn_name = rest[1].clone();
        let local = binding.rename.unwrap_or_else(|| fn_name.clone());
        remote_fn_locals.insert(local, (interface.clone(), fn_name));
        return Ok(None);
    }

    let (module_path, item_segments) = split_crate_path(rest);

    // `use provider_api::remote` / `use provider_api::remote::*` - remote.call stubs,
    // not Lua requires.
    if let Some(interface) = factorio_binding.interface.as_ref()
        && module_path == "remote"
    {
        let local = binding.rename.unwrap_or_else(|| "remote".to_string());
        remote_locals.insert(local, interface.clone());
        return Ok(None);
    }

    if module_path.is_empty() {
        return Err(FrontendError::UnsupportedItem {
            item: format!("use {}", binding.segments.join("::")),
            location: factorio_ir::span::SourceLoc::default().with_note("use"),
        });
    }

    Ok(Some(fragment_from_parts(
        module_path,
        &item_segments,
        binding.rename,
        module_prefix,
        Some(factorio_binding.mod_name.clone()),
        Some(factorio_binding.module_root.clone()),
    )?))
}

fn fragment_from_parts(
    module_path: String,
    item_segments: &[String],
    rename: Option<String>,
    module_prefix: &str,
    factorio_mod: Option<String>,
    module_root: Option<String>,
) -> FrontendResult<ImportFragment> {
    if item_segments.is_empty() {
        let default_local = module_path
            .rsplit('.')
            .next()
            .map_or_else(|| require_local_name(&module_path), str::to_string);
        let prefixed_local = apply_prefix(&default_local, module_prefix);
        return Ok(ImportFragment {
            module: module_path,
            require_local: rename.unwrap_or(prefixed_local),
            item: None,
            factorio_mod,
            module_root,
        });
    }

    if item_segments.len() == 1 {
        #[allow(clippy::indexing_slicing)]
        let item_name = item_segments[0].clone();
        let prefixed_local = apply_prefix(&require_local_name(&module_path), module_prefix);
        return Ok(ImportFragment {
            module: module_path,
            require_local: prefixed_local,
            item: Some(factorio_ir::module::ImportedItem {
                name: item_name.clone(),
                local: rename.unwrap_or(item_name),
            }),
            factorio_mod,
            module_root,
        });
    }

    Err(FrontendError::UnsupportedItem {
        item: format!(
            "use {}{}{}",
            factorio_mod.as_deref().unwrap_or("crate"),
            if module_path.is_empty() { "" } else { "::" },
            std::iter::once(module_path.as_str())
                .chain(item_segments.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join("::")
        ),
        location: factorio_ir::span::SourceLoc::default().with_note("use"),
    })
}

pub fn merge_imports(
    fragments: Vec<ImportFragment>,
    module_prefix: &str,
) -> Vec<factorio_ir::module::ModuleImport> {
    let mut merged = BTreeMap::<String, factorio_ir::module::ModuleImport>::new();

    for fragment in fragments {
        let key = format!(
            "{}::{}",
            fragment.factorio_mod.as_deref().unwrap_or(""),
            fragment.module
        );
        let entry = merged
            .entry(key)
            .or_insert_with(|| factorio_ir::module::ModuleImport {
                module: fragment.module.clone(),
                local: apply_prefix(&require_local_name(&fragment.module), module_prefix),
                items: Vec::new(),
                factorio_mod: fragment.factorio_mod.clone(),
                module_root: fragment.module_root.clone(),
            });

        if fragment.item.is_none() {
            entry.local = fragment.require_local;
        }

        if let Some(item) = fragment.item
            && !entry
                .items
                .iter()
                .any(|existing| existing.local == item.local)
        {
            entry.items.push(item);
        }
    }

    merged.into_values().collect()
}

fn apply_prefix(local: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}_{local}")
    }
}
