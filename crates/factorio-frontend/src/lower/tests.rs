use std::collections::HashMap;
use std::path::{Path, PathBuf};

use factorio_ir::lint::Diagnostic;
use syn::Item;

use crate::{
    FrontendError, FrontendResult,
    lower::{
        ParseOptions, attrs,
        context::LowerContext,
        functions::lower_function,
        imports::{ImportFragment, lower_use, merge_imports},
        util,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct FactorioTest {
    /// Qualified report name, e.g. `tests::building_explodes_when_health_is_zero`.
    pub name: String,
    /// Sanitized Lua function name.
    pub lua_name: String,
    pub function: factorio_ir::function::Function,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestSuite {
    pub tests: Vec<FactorioTest>,
    pub helpers: Vec<factorio_ir::statement::Statement>,
    pub imports: Vec<factorio_ir::module::ModuleImport>,
}

impl TestSuite {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tests.is_empty()
    }

    #[must_use]
    pub fn to_module(&self) -> factorio_ir::module::Module {
        let mut symbols = Vec::new();
        for test in &self.tests {
            let mut function = test.function.clone();
            function.name.clone_from(&test.lua_name);
            symbols.push(factorio_ir::module::Symbol {
                scope: factorio_ir::scope::Scope::Public,
                statement: factorio_ir::statement::Statement::FunctionDecl(function),
            });
        }
        factorio_ir::module::Module {
            name: "factorio_rs_tests".to_string(),
            stage: factorio_ir::stage::Stage::Control,
            body: factorio_ir::block::Block {
                statements: self.helpers.clone(),
            },
            symbols,
            imports: self.imports.clone(),
            submodules: vec![],
            locales: vec![],
            pending_locales: vec![],
        }
    }
}

/// Discover `#[test]` functions under `#[cfg(test)]` modules in `sources`.
///
/// `source_dir` is the project's `src/` root (used to resolve `mod tests;` files).
///
/// # Errors
/// Returns lowering / parse failures for test modules.
pub fn discover_tests(
    source_dir: &Path,
    sources: &[(PathBuf, String)],
    options: &ParseOptions<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> FrontendResult<TestSuite> {
    let mut tests = Vec::new();
    let mut helpers = Vec::new();
    let mut import_fragments = Vec::new();
    let mut seen_lua_names = HashMap::<String, usize>::new();

    for (path, source) in sources {
        let file = syn::parse_file(source)?;
        collect_from_items(
            &file.items,
            source_dir,
            path,
            "",
            options,
            diagnostics,
            &mut tests,
            &mut helpers,
            &mut seen_lua_names,
            &mut import_fragments,
        )?;
    }

    Ok(TestSuite {
        tests,
        helpers,
        imports: merge_imports(import_fragments, options.module_prefix),
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_from_items(
    items: &[Item],
    source_dir: &Path,
    source_path: &Path,
    path_prefix: &str,
    options: &ParseOptions<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    tests: &mut Vec<FactorioTest>,
    helpers: &mut Vec<factorio_ir::statement::Statement>,
    seen_lua_names: &mut HashMap<String, usize>,
    import_fragments: &mut Vec<ImportFragment>,
) -> FrontendResult<()> {
    for item in items {
        match item {
            Item::Macro(item_macro)
                if attrs::is_factorio_stage_bang(&item_macro.mac.path).is_some() =>
            {
                let nested = parse_macro_items(item_macro.mac.tokens.clone())?;
                collect_from_items(
                    &nested,
                    source_dir,
                    source_path,
                    path_prefix,
                    options,
                    diagnostics,
                    tests,
                    helpers,
                    seen_lua_names,
                    import_fragments,
                )?;
            }
            Item::Mod(item_mod) if attrs::is_cfg_test(&item_mod.attrs) => {
                let mod_name = item_mod.ident.to_string();
                let nested_prefix = if path_prefix.is_empty() {
                    mod_name.clone()
                } else {
                    format!("{path_prefix}::{mod_name}")
                };

                if let Some((_, nested)) = &item_mod.content {
                    lower_test_module_items(
                        nested,
                        &nested_prefix,
                        options,
                        diagnostics,
                        tests,
                        helpers,
                        seen_lua_names,
                        import_fragments,
                    )?;
                } else {
                    let child_path = resolve_mod_file(source_dir, source_path, &mod_name)?;
                    let child_source = std::fs::read_to_string(&child_path).map_err(|source| {
                        FrontendError::Syn(format!(
                            "failed to read test module `{}`: {source}",
                            child_path.display()
                        ))
                    })?;
                    let child_file = syn::parse_file(&child_source)?;
                    lower_test_module_items(
                        &child_file.items,
                        &nested_prefix,
                        options,
                        diagnostics,
                        tests,
                        helpers,
                        seen_lua_names,
                        import_fragments,
                    )?;
                }
            }
            Item::Mod(item_mod) => {
                if let Some((_, nested)) = &item_mod.content {
                    collect_from_items(
                        nested,
                        source_dir,
                        source_path,
                        path_prefix,
                        options,
                        diagnostics,
                        tests,
                        helpers,
                        seen_lua_names,
                        import_fragments,
                    )?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_test_module_items(
    items: &[Item],
    path_prefix: &str,
    options: &ParseOptions<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    tests: &mut Vec<FactorioTest>,
    helpers: &mut Vec<factorio_ir::statement::Statement>,
    seen_lua_names: &mut HashMap<String, usize>,
    import_fragments: &mut Vec<ImportFragment>,
) -> FrontendResult<()> {
    let mut remote_locals = HashMap::new();
    let mut remote_fn_locals = HashMap::new();

    for item in items {
        if let Item::Use(use_item) = item {
            let fragments = lower_use(
                use_item,
                options.module_prefix,
                options.bindings(),
                &mut remote_locals,
                &mut remote_fn_locals,
            )?;
            import_fragments.extend(fragments);
        }
    }

    let mut bare_import_renames = HashMap::new();
    if !options.module_prefix.is_empty() {
        for fragment in import_fragments.iter() {
            if fragment.item.is_none() {
                let bare = crate::paths::require_local_name(&fragment.module);
                if bare != fragment.require_local {
                    bare_import_renames.insert(bare, fragment.require_local.clone());
                }
            }
        }
    }

    let mut ctx = LowerContext {
        imports: import_fragments,
        module_prefix: options.module_prefix,
        bindings: options.bindings(),
        bare_import_renames,
        remote_locals,
        remote_fn_locals,
        binding_types: HashMap::new(),
        enums: HashMap::new(),
        type_aliases: HashMap::new(),
        option_bindings: std::collections::HashSet::new(),
        lints: options.lints,
        diagnostics,
        try_hoists: Vec::new(),
        try_tmp_counter: 0,
    };
    super::types::collect_type_aliases(items, &mut ctx.type_aliases)?;

    let mut nested_mods: Vec<(String, &[Item])> = Vec::new();

    for item in items {
        match item {
            Item::Fn(function) if attrs::is_test_fn(&function.attrs) => {
                let lowered = lower_function(function, &mut ctx)?;
                let report_name = if path_prefix.is_empty() {
                    lowered.name.clone()
                } else {
                    format!("{path_prefix}::{}", lowered.name)
                };
                let lua_name = unique_lua_name(&lowered.name, seen_lua_names);
                tests.push(FactorioTest {
                    name: report_name,
                    lua_name,
                    function: lowered,
                });
            }
            Item::Fn(function) => {
                let lowered = lower_function(function, &mut ctx)?;
                helpers.push(factorio_ir::statement::Statement::FunctionDecl(lowered));
            }
            Item::Mod(nested) => {
                if let Some((_, nested_items)) = &nested.content {
                    let mod_name = nested.ident.to_string();
                    nested_mods.push((mod_name, nested_items.as_slice()));
                }
            }
            Item::Use(_) => {}
            other => {
                return Err(FrontendError::UnsupportedItem {
                    item: util::item_name(other),
                    location: util::location(other).with_note(
                        "only fns, use, and nested mods are supported in #[cfg(test)] modules",
                    ),
                });
            }
        }
    }
    drop(ctx);

    for (mod_name, nested_items) in nested_mods {
        let nested_prefix = format!("{path_prefix}::{mod_name}");
        lower_test_module_items(
            nested_items,
            &nested_prefix,
            options,
            diagnostics,
            tests,
            helpers,
            seen_lua_names,
            import_fragments,
        )?;
    }

    Ok(())
}

fn unique_lua_name(base: &str, seen: &mut HashMap<String, usize>) -> String {
    let count = seen.entry(base.to_string()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base.to_string()
    } else {
        format!("{base}_{count}")
    }
}

fn resolve_mod_file(
    source_dir: &Path,
    parent_path: &Path,
    mod_name: &str,
) -> FrontendResult<PathBuf> {
    let parent_dir = parent_path.parent().unwrap_or(source_dir);
    let candidates = [
        parent_dir.join(format!("{mod_name}.rs")),
        parent_dir.join(mod_name).join("mod.rs"),
    ];
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    Err(FrontendError::Syn(format!(
        "could not find test module file for `mod {mod_name}` (looked next to {})",
        parent_path.display()
    )))
}

fn parse_macro_items(tokens: proc_macro2::TokenStream) -> FrontendResult<Vec<Item>> {
    let file = syn::parse2::<syn::File>(tokens).map_err(FrontendError::from)?;
    Ok(file.items)
}

#[cfg(test)]
mod unit_tests {
    #![allow(clippy::unwrap_used)]

    use factorio_ir::lint::LintConfig;

    use super::*;
    use crate::lower::ParseOptions;

    #[test]
    fn discovers_inline_cfg_test_module() {
        let source = r"
            #[factorio_rs::control]
            mod control {
                pub fn helper() {}

                #[cfg(test)]
                mod tests {
                    #[test]
                    fn adds_one() {
                        assert_eq!(1 + 1, 2);
                    }

                    #[test]
                    fn truth() {
                        assert!(true);
                    }
                }
            }
        ";
        let lints = LintConfig::allow_all();
        let options = ParseOptions::new(&lints);
        let mut diagnostics = Vec::new();
        let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
        let suite = discover_tests(Path::new("src"), &sources, &options, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty());
        assert_eq!(suite.tests.len(), 2);
        assert_eq!(suite.tests[0].name, "tests::adds_one");
        assert_eq!(suite.tests[1].name, "tests::truth");
    }

    #[test]
    fn discovers_tests_inside_control_mod_bang() {
        let source = r"
            factorio_rs::control_mod! {
                #[cfg(test)]
                mod tests {
                    #[test]
                    fn smoke() {
                        assert_eq!(1, 1);
                    }
                }
            }
        ";
        let lints = LintConfig::allow_all();
        let options = ParseOptions::new(&lints);
        let mut diagnostics = Vec::new();
        let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
        let suite = discover_tests(Path::new("src"), &sources, &options, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty());
        assert_eq!(suite.tests.len(), 1);
        assert_eq!(suite.tests[0].name, "tests::smoke");
    }

    #[test]
    fn lowers_factorio_rs_test_steps_to_intrinsic() {
        let source = r"
            #[cfg(test)]
            mod tests {
                #[test]
                fn tick_advances() {
                    factorio_rs::test::steps()
                        .step(|_ctx| {})
                        .wait(5)
                        .step(|_ctx| {});
                }
            }
        ";
        let lints = LintConfig::allow_all();
        let options = ParseOptions::new(&lints);
        let mut diagnostics = Vec::new();
        let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
        let suite = discover_tests(Path::new("src"), &sources, &options, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty());
        assert_eq!(suite.tests.len(), 1);
        let module = suite.to_module();
        let lua = factorio_codegen::LuaGenerator::with_mod_name("test_mod")
            .generate_module(&module)
            .unwrap();
        assert!(
            lua.contains("__frs_steps()"),
            "expected __frs_steps intrinsic in:\n{lua}"
        );
        assert!(lua.contains(".wait(5)"), "expected wait in:\n{lua}");
    }
}
