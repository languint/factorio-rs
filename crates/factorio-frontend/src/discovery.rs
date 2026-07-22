use std::path::Path;

use syn::{File, Item};

use factorio_ir::{function::ExportMeta, stage::Stage};

use crate::{
    error::{FrontendError, FrontendResult},
    lower::attrs::{
        extract_factorio_stage, is_factorio_stage_bang, parse_factorio_export_attribute,
    },
    paths::module_name_from_source,
};

/// A transpilable module discovered in a source file.
#[derive(Clone)]
pub struct DiscoveredModule {
    pub module_name: String,
    pub stage: Stage,
    pub items: Vec<Item>,
    /// When the module itself carries `#[factorio_rs::export]`, every `pub fn`
    /// without its own export attribute inherits this metadata.
    pub default_export: Option<ExportMeta>,
}

/// Discover transpilable modules in a source file.
///
/// Modules are found via:
/// - path-based layout (`src/control/...`, `src/control.rs`, ...)
/// - `factorio_rs::control_mod! { ... }` block macros (and `shared_mod!` / `data_mod!` /
///   `settings_updates_mod!` / ...)
/// - `#[factorio_rs::control] mod name { ... }` inline modules
///
/// # Errors
/// Returns `Err` if parsing the Rust source fails.
pub fn discover_modules(
    source_dir: &Path,
    source_path: &Path,
    source: &str,
) -> FrontendResult<Vec<DiscoveredModule>> {
    let file = syn::parse_file(source)?;

    if let Some(module_name) = module_name_from_source(source_dir, source_path) {
        // Any file under src/ is transpilable. Files whose names don't match a
        // known stage prefix (control/settings*/data*/shared) default to Shared so
        // that helper modules like `adjacent_blacklist.rs` don't need to live
        // inside a `shared/` subdirectory.
        let stage = Stage::from_module_name(&module_name).unwrap_or(Stage::Shared);
        return Ok(vec![DiscoveredModule {
            module_name,
            stage,
            items: file.items,
            default_export: file.attrs.iter().find_map(parse_factorio_export_attribute),
        }]);
    }

    let mut discovered = Vec::new();

    if let Some(stage) = extract_factorio_stage(&file.attrs) {
        discovered.push(DiscoveredModule {
            module_name: stage.default_module_name().to_string(),
            stage,
            items: file.items,
            default_export: file.attrs.iter().find_map(parse_factorio_export_attribute),
        });
        return Ok(discovered);
    }

    for item in file.items {
        match item {
            Item::Macro(item_macro) => {
                if let Some(stage) = is_factorio_stage_bang(&item_macro.mac.path) {
                    let items = parse_macro_items(item_macro.mac.tokens)?;
                    discovered.push(DiscoveredModule {
                        module_name: stage.default_module_name().to_string(),
                        stage,
                        items,
                        default_export: None,
                    });
                }
            }
            Item::Mod(item_mod) => {
                if let Some(stage) = extract_factorio_stage(&item_mod.attrs) {
                    let Some((_, items)) = item_mod.content else {
                        continue;
                    };
                    discovered.push(DiscoveredModule {
                        module_name: item_mod.ident.to_string(),
                        stage,
                        items,
                        default_export: item_mod
                            .attrs
                            .iter()
                            .find_map(parse_factorio_export_attribute),
                    });
                }
            }
            _ => {}
        }
    }

    Ok(discovered)
}

/// Discover transpilable modules from a rustc-expanded crate (`-Zunpretty=expanded`).
///
/// Walks nested `mod` items, inferring stage from `__factorio_rs_stage` markers,
/// stage attributes, or the dotted module path (`control`, `shared.api`, ...).
///
/// # Errors
/// Returns `Err` if parsing the expanded Rust source fails.
pub fn discover_modules_from_expanded(source: &str) -> FrontendResult<Vec<DiscoveredModule>> {
    let file = syn::parse_file(source)?;
    let mut discovered = Vec::new();

    if let Some(stage) = root_stage_from_markers(&file.items) {
        let items: Vec<Item> = file
            .items
            .iter()
            .filter(|item| !matches!(item, Item::Mod(_) | Item::ExternCrate(_) | Item::Use(_)))
            .cloned()
            .collect();
        if items.iter().any(is_lowerable_item) {
            discovered.push(DiscoveredModule {
                module_name: stage.default_module_name().to_string(),
                stage,
                items,
                default_export: file.attrs.iter().find_map(parse_factorio_export_attribute),
            });
        }
    }

    walk_expanded_items(&file.items, "", None, &mut discovered);
    Ok(discovered)
}

fn root_stage_from_markers(items: &[Item]) -> Option<Stage> {
    crate::lower::meta_markers::collect_module_meta_markers(items)
        .stage
        .as_deref()
        .and_then(stage_from_marker)
}

fn is_lowerable_item(item: &Item) -> bool {
    match item {
        Item::Mod(_) | Item::Use(_) | Item::ExternCrate(_) | Item::Verbatim(_) => false,
        Item::Const(c) if crate::lower::meta_markers::is_meta_marker_const(c) => false,
        Item::Macro(mac) if mac.mac.path.is_ident("macro_rules") => false,
        _ => true,
    }
}

fn walk_expanded_items(
    items: &[Item],
    prefix: &str,
    inherited_stage: Option<Stage>,
    discovered: &mut Vec<DiscoveredModule>,
) {
    for item in items {
        let Item::Mod(item_mod) = item else {
            continue;
        };
        let name = item_mod.ident.to_string();
        if name == "factorio_exports" {
            continue;
        }
        let raw_module_name = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };

        let Some((_, content)) = &item_mod.content else {
            continue;
        };

        let stage = extract_factorio_stage(&item_mod.attrs)
            .or_else(|| {
                crate::lower::meta_markers::collect_module_meta_markers(content)
                    .stage
                    .as_deref()
                    .and_then(stage_from_marker)
            })
            .or_else(|| stage_from_bang_wrapper(&name))
            .or_else(|| Stage::from_module_name(&raw_module_name))
            .or(inherited_stage)
            .unwrap_or(Stage::Shared);

        // `control_mod!` expands to `mod __factorio_control { ... }`; emit the
        // canonical stage module name (`control`) for Lua paths / manifests.
        let module_name = canonicalize_bang_wrapper_name(&raw_module_name, stage);

        let has_nested_mods = content.iter().any(|nested| matches!(nested, Item::Mod(_)));
        let has_lowerable = content.iter().any(is_lowerable_item);

        if has_lowerable {
            let items: Vec<Item> = content
                .iter()
                .map(|nested| match nested {
                    // Keep `mod child;` so the parent still emits requires; bodies are
                    // discovered separately via recursion.
                    Item::Mod(item_mod) => Item::Mod(syn::ItemMod {
                        attrs: item_mod.attrs.clone(),
                        vis: item_mod.vis.clone(),
                        unsafety: item_mod.unsafety,
                        mod_token: item_mod.mod_token,
                        ident: item_mod.ident.clone(),
                        content: None,
                        semi: Some(syn::token::Semi::default()),
                    }),
                    other => other.clone(),
                })
                .collect();
            discovered.push(DiscoveredModule {
                module_name: module_name.clone(),
                stage,
                items,
                default_export: item_mod
                    .attrs
                    .iter()
                    .find_map(parse_factorio_export_attribute),
            });
        }

        if has_nested_mods {
            walk_expanded_items(content, &module_name, Some(stage), discovered);
        }
    }
}

/// `control_mod!` / `data_mod!` / ... expand to `mod __factorio_{stage}`.
fn stage_from_bang_wrapper(name: &str) -> Option<Stage> {
    name.strip_prefix("__factorio_").and_then(stage_from_marker)
}

/// Map `__factorio_control` / `__factorio_control.nested` -> `control` / `control.nested`.
fn canonicalize_bang_wrapper_name(module_name: &str, stage: Stage) -> String {
    let wrapper = format!("__factorio_{}", stage.default_module_name());
    if module_name == wrapper {
        return stage.default_module_name().to_string();
    }
    if let Some(rest) = module_name.strip_prefix(&wrapper)
        && let Some(nested) = rest.strip_prefix('.')
    {
        return format!("{}.{nested}", stage.default_module_name());
    }
    module_name.to_string()
}

fn stage_from_marker(marker: &str) -> Option<Stage> {
    match marker {
        "settings" => Some(Stage::Settings),
        "settings_updates" => Some(Stage::SettingsUpdates),
        "settings_final_fixes" => Some(Stage::SettingsFinalFixes),
        "data" => Some(Stage::Data),
        "data_updates" => Some(Stage::DataUpdates),
        "data_final_fixes" => Some(Stage::DataFinalFixes),
        "control" => Some(Stage::Control),
        "shared" => Some(Stage::Shared),
        _ => None,
    }
}

fn parse_macro_items(tokens: proc_macro2::TokenStream) -> FrontendResult<Vec<Item>> {
    let file = syn::parse2::<File>(tokens).map_err(FrontendError::from)?;
    Ok(file.items)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{discover_modules, discover_modules_from_expanded};

    #[test]
    fn discovers_control_block_macro_in_lib_rs() {
        let source_dir = Path::new("/project/src");
        let source_path = source_dir.join("lib.rs");
        let source = r"
            factorio_rs::control_mod! {
                #[factorio_rs::event(OnSingleplayerInit)]
                pub fn on_singleplayer_init() {}
            }
        ";

        let modules = discover_modules(source_dir, &source_path, source).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_name, "control");
        assert_eq!(modules[0].stage, factorio_ir::stage::Stage::Control);
    }

    #[test]
    fn discovers_expanded_control_mod_bang_wrapper() {
        // Shape produced by `control_mod!` after rustc `-Zunpretty=expanded`.
        let expanded = r#"
            #[doc(hidden)]
            mod __factorio_control {
                #[doc(hidden)]
                #[allow(non_upper_case_globals)]
                const __factorio_rs_stage: &str = "control";

                #[allow(dead_code)]
                pub fn on_singleplayer_init() {}

                #[doc(hidden)]
                #[allow(non_upper_case_globals)]
                pub const __factorio_rs_event__on_singleplayer_init: &str = "";
            }
        "#;

        let modules = discover_modules_from_expanded(expanded).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_name, "control");
        assert_eq!(modules[0].stage, factorio_ir::stage::Stage::Control);
    }

    #[test]
    fn discovers_expanded_control_mod_bang_wrapper_by_name_alone() {
        // Even without the stage marker const, the `__factorio_{stage}` wrapper
        // name is enough to recover the stage.
        let expanded = r"
            #[doc(hidden)]
            mod __factorio_control {
                pub fn on_singleplayer_init() {}
            }
        ";

        let modules = discover_modules_from_expanded(expanded).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_name, "control");
        assert_eq!(modules[0].stage, factorio_ir::stage::Stage::Control);
    }

    #[test]
    fn discovers_inner_control_attribute_on_lib_rs() {
        let source_dir = Path::new("/project/src");
        let source_path = source_dir.join("lib.rs");
        let source = r"
            #![factorio_rs::control]

            #[factorio_rs::event(OnSingleplayerInit)]
            pub fn on_singleplayer_init() {}
        ";

        let modules = discover_modules(source_dir, &source_path, source).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_name, "control");
        assert_eq!(modules[0].stage, factorio_ir::stage::Stage::Control);
    }

    #[test]
    fn discovers_attributed_inline_mod() {
        let source_dir = Path::new("/project/src");
        let source_path = source_dir.join("lib.rs");
        let source = r"
            #[factorio_rs::control]
            mod handlers {
                pub fn on_init() {}
            }
        ";

        let modules = discover_modules(source_dir, &source_path, source).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_name, "handlers");
        assert_eq!(modules[0].stage, factorio_ir::stage::Stage::Control);
    }

    #[test]
    fn discovers_settings_phase_modules_by_path() {
        let source_dir = Path::new("/project/src");

        let updates = discover_modules(
            source_dir,
            &source_dir.join("settings_updates.rs"),
            "pub fn patch() {}",
        )
        .unwrap();
        assert_eq!(updates[0].module_name, "settings_updates");
        assert_eq!(updates[0].stage, factorio_ir::stage::Stage::SettingsUpdates);

        let finals = discover_modules(
            source_dir,
            &source_dir.join("settings_final_fixes.rs"),
            "pub fn fixup() {}",
        )
        .unwrap();
        assert_eq!(finals[0].module_name, "settings_final_fixes");
        assert_eq!(
            finals[0].stage,
            factorio_ir::stage::Stage::SettingsFinalFixes
        );
    }

    #[test]
    fn discovers_data_phase_attribute() {
        let source_dir = Path::new("/project/src");
        let source_path = source_dir.join("lib.rs");
        let source = r"
            #![factorio_rs::data_updates]

            pub fn patch() {}
        ";

        let modules = discover_modules(source_dir, &source_path, source).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_name, "data_updates");
        assert_eq!(modules[0].stage, factorio_ir::stage::Stage::DataUpdates);
    }

    #[test]
    fn discovers_expanded_helper_modules_as_shared() {
        let expanded = r"
            mod force_state {
                pub struct Milestone {
                    pub name: String,
                }
                pub fn load(force_name: &str) -> Vec<Milestone> {
                    Vec::new()
                }
            }

            mod control {
                use crate::force_state::Milestone;
                pub fn on_tick() {}
            }
        ";

        let modules = discover_modules_from_expanded(expanded).unwrap();
        let names: Vec<_> = modules.iter().map(|m| m.module_name.as_str()).collect();
        assert!(names.contains(&"force_state"), "got {names:?}");
        assert!(names.contains(&"control"), "got {names:?}");

        let force_state = modules
            .iter()
            .find(|m| m.module_name == "force_state")
            .expect("force_state");
        assert_eq!(force_state.stage, factorio_ir::stage::Stage::Shared);

        let control = modules
            .iter()
            .find(|m| m.module_name == "control")
            .expect("control");
        assert_eq!(control.stage, factorio_ir::stage::Stage::Control);
    }
}
