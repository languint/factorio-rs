use std::collections::BTreeMap;

use syn::{ItemUse, UseGroup, UseName, UsePath, UseRename, UseTree};

use crate::{
    error::{FrontendError, FrontendResult},
    paths::{require_local_name, split_crate_path},
};

pub struct ImportFragment {
    pub module: String,
    pub require_local: String,
    pub item: Option<factorio_ir::module::ImportedItem>,
}

struct RawUseBinding {
    segments: Vec<String>,
    rename: Option<String>,
}

pub fn lower_use(item: &ItemUse, module_prefix: &str) -> FrontendResult<Vec<ImportFragment>> {
    let mut bindings = Vec::new();
    collect_use_bindings(&item.tree, &mut Vec::new(), &mut bindings)?;

    let mut fragments = Vec::new();
    for binding in bindings {
        if let Some(fragment) = finalize_use_binding(binding, module_prefix)? {
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
            // `use crate::foo::*` → record a module-level import of `crate::foo`
            // so the module gets `require`d in Lua.
            // External globs like `use factorio_rs::prelude::*` are filtered out
            // later by `finalize_use_binding` because they don't start with `crate`.
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
) -> FrontendResult<Option<ImportFragment>> {
    if binding.segments.first().map(String::as_str) != Some("crate") {
        return Ok(None);
    }

    let (module_path, item_segments) = split_crate_path(&binding.segments[1..]);
    if module_path.is_empty() {
        return Err(FrontendError::UnsupportedItem {
            item: format!("use {}", binding.segments.join("::")),
            location: "use".to_string(),
        });
    }

    if item_segments.is_empty() {
        // The local Lua name for a bare module import (`use crate::shared::foo`)
        // is the last segment of the binding path (i.e. `foo`), matching Rust's
        // scoping rules. The full dotted path is used only for the `require()` call.
        let default_local = binding
            .segments
            .last()
            .cloned()
            .unwrap_or_else(|| require_local_name(&module_path));
        let prefixed_local = apply_prefix(&default_local, module_prefix);
        return Ok(Some(ImportFragment {
            module: module_path,
            require_local: binding.rename.unwrap_or(prefixed_local),
            item: None,
        }));
    }

    if item_segments.len() == 1 {
        let prefixed_local = apply_prefix(&require_local_name(&module_path), module_prefix);
        return Ok(Some(ImportFragment {
            module: module_path,
            require_local: prefixed_local,
            item: Some(factorio_ir::module::ImportedItem {
                name: item_segments[0].clone(),
                local: binding.rename.unwrap_or_else(|| item_segments[0].clone()),
            }),
        }));
    }

    Err(FrontendError::UnsupportedItem {
        item: format!("use {}", binding.segments.join("::")),
        location: "use".to_string(),
    })
}

pub fn merge_imports(
    fragments: Vec<ImportFragment>,
    module_prefix: &str,
) -> Vec<factorio_ir::module::ModuleImport> {
    let mut merged = BTreeMap::<String, factorio_ir::module::ModuleImport>::new();

    for fragment in fragments {
        let entry = merged.entry(fragment.module.clone()).or_insert_with(|| {
            factorio_ir::module::ModuleImport {
                module: fragment.module.clone(),
                local: apply_prefix(&require_local_name(&fragment.module), module_prefix),
                items: Vec::new(),
            }
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
