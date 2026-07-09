use std::path::Path;

use syn::{File, Item};

use factorio_ir::stage::Stage;

use crate::{
    error::{FrontendError, FrontendResult},
    lower::attrs::{extract_factorio_stage, is_factorio_stage_bang},
    paths::module_name_from_source,
};

/// A transpilable module discovered in a source file.
#[derive(Clone)]
pub struct DiscoveredModule {
    pub module_name: String,
    pub stage: Stage,
    pub items: Vec<Item>,
}

/// Discover transpilable modules in a source file.
///
/// Modules are found via:
/// - path-based layout (`src/control/...`, `src/control.rs`, ...)
/// - `factorio::control_mod! { ... }` block macros (and `shared_mod!` / `data_mod!`)
/// - `#[factorio::control] mod name { ... }` inline modules
pub fn discover_modules(
    source_dir: &Path,
    source_path: &Path,
    source: &str,
) -> FrontendResult<Vec<DiscoveredModule>> {
    let file = syn::parse_file(source)?;

    if let Some(module_name) = module_name_from_source(source_dir, source_path)
        && let Some(stage) = Stage::from_module_name(&module_name)
    {
        return Ok(vec![DiscoveredModule {
            module_name,
            stage,
            items: file.items,
        }]);
    }

    let mut discovered = Vec::new();

    if let Some(stage) = extract_factorio_stage(&file.attrs) {
        discovered.push(DiscoveredModule {
            module_name: stage.default_module_name().to_string(),
            stage,
            items: file.items,
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
                    });
                }
            }
            _ => {}
        }
    }

    Ok(discovered)
}

fn parse_macro_items(tokens: proc_macro2::TokenStream) -> FrontendResult<Vec<Item>> {
    let file = syn::parse2::<File>(tokens).map_err(FrontendError::from)?;
    Ok(file.items)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::discover_modules;

    #[test]
    fn discovers_control_block_macro_in_lib_rs() {
        let source_dir = Path::new("/project/src");
        let source_path = source_dir.join("lib.rs");
        let source = r"
            factorio::control_mod! {
                #[factorio::event(OnInit)]
                pub fn on_init() {}
            }
        ";

        let modules = discover_modules(source_dir, &source_path, source).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_name, "control");
        assert_eq!(modules[0].stage, factorio_ir::stage::Stage::Control);
    }

    #[test]
    fn discovers_inner_control_attribute_on_lib_rs() {
        let source_dir = Path::new("/project/src");
        let source_path = source_dir.join("lib.rs");
        let source = r#"
            #![factorio::control]

            #[factorio::event(OnInit)]
            pub fn on_init() {}
        "#;

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
            #[factorio::control]
            mod handlers {
                pub fn on_init() {}
            }
        ";

        let modules = discover_modules(source_dir, &source_path, source).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_name, "handlers");
        assert_eq!(modules[0].stage, factorio_ir::stage::Stage::Control);
    }
}
