use std::collections::{BTreeMap, HashMap, HashSet};
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
    },
};

/// A single discovered bench function.
#[derive(Debug, Clone, PartialEq)]
pub struct FactorioBench {
    /// Qualified report name, e.g. `benches::my_heavy_bench`.
    pub name: String,
    /// Sanitised Lua function name (unique within the suite).
    pub lua_name: String,
    /// Number of times the bench body should run per measurement (>= 1).
    pub iterations: u32,
    pub function: factorio_ir::function::Function,
}

/// Collection of all discovered benches plus their shared helpers.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchSuite {
    pub benches: Vec<FactorioBench>,
    pub helpers: Vec<factorio_ir::statement::Statement>,
    pub imports: Vec<factorio_ir::module::ModuleImport>,
    pub vtables: Vec<factorio_ir::module::VTable>,
}

impl BenchSuite {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.benches.is_empty()
    }

    /// Produce a module named `"factorio_rs_benches"` containing all bench
    /// functions as public symbols and the shared helpers in the module body.
    #[must_use]
    pub fn to_module(&self) -> factorio_ir::module::Module {
        let mut symbols = Vec::new();
        for bench in &self.benches {
            let mut function = bench.function.clone();
            function.name.clone_from(&bench.lua_name);
            symbols.push(factorio_ir::module::Symbol {
                scope: factorio_ir::scope::Scope::Public,
                statement: factorio_ir::statement::Statement::FunctionDecl(function),
            });
        }
        factorio_ir::module::Module {
            name: "factorio_rs_benches".to_string(),
            stage: factorio_ir::stage::Stage::Control,
            body: factorio_ir::block::Block {
                statements: self.helpers.clone(),
            },
            symbols,
            imports: self.imports.clone(),
            submodules: vec![],
            locales: vec![],
            pending_locales: vec![],
            vtables: self.vtables.clone(),
        }
    }
}

/// Discover `#[factorio_rs::bench]` functions in `sources`.
///
/// Benches may live anywhere: at the top level of a control module, inside a
/// `#[cfg(test)]` module, or in any nested `mod`.  `source_dir` is the
/// project's `src/` root (currently unused for file-mod resolution; sources
/// should already include all relevant files).
///
/// Parent-module structs, inherent/trait impls, free functions (except event
/// handlers and other bench fns), trait vtables, and `use` imports are
/// lowered into the suite so bench fns can call helpers from the same scope.
///
/// # Errors
/// Returns lowering / parse failures encountered while walking sources.
pub fn discover_benches(
    _source_dir: &Path,
    sources: &[(PathBuf, String)],
    options: &ParseOptions<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> FrontendResult<BenchSuite> {
    let mut benches = Vec::new();
    let mut helpers = Vec::new();
    let mut vtables = Vec::new();
    let mut import_fragments = Vec::new();
    let mut parent_imports = Vec::new();
    let mut seen_lua_names = HashMap::<String, usize>::new();

    for (_path, source) in sources {
        let file = syn::parse_file(source)?;
        collect_from_items(
            &file.items,
            // source_dir,
            // path,
            "",
            options,
            diagnostics,
            &mut benches,
            &mut helpers,
            &mut vtables,
            &mut seen_lua_names,
            &mut import_fragments,
            &mut parent_imports,
        )?;
    }

    let mut imports = merge_imports(import_fragments, options.module_prefix);
    merge_parent_imports(&mut imports, parent_imports);

    Ok(BenchSuite {
        benches,
        helpers,
        imports,
        vtables,
    })
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn collect_from_items(
    items: &[Item],
    // _source_dir: &Path,
    // _source_path: &Path,
    path_prefix: &str,
    options: &ParseOptions<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    benches: &mut Vec<FactorioBench>,
    helpers: &mut Vec<factorio_ir::statement::Statement>,
    vtables: &mut Vec<factorio_ir::module::VTable>,
    seen_lua_names: &mut HashMap<String, usize>,
    import_fragments: &mut Vec<ImportFragment>,
    parent_imports: &mut Vec<factorio_ir::module::ModuleImport>,
) -> FrontendResult<()> {
    // Collect bench fns at this scope level (order preserved).
    let bench_fns_at_level: Vec<&syn::ItemFn> = items
        .iter()
        .filter_map(|item| {
            if let Item::Fn(f) = item
                && attrs::is_bench_fn(&f.attrs)
            {
                return Some(f);
            }
            None
        })
        .collect();

    if !bench_fns_at_level.is_empty() {
        let (level_helpers, level_vtables, level_imports) =
            lower_parent_support_items(items, options, diagnostics)?;
        merge_parent_helpers(helpers, level_helpers);
        merge_parent_vtables(vtables, level_vtables);
        parent_imports.extend(level_imports);

        // Collect `use` items at this scope level to feed the context.
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

        // Lower each bench fn in its own short-lived context so that
        // `diagnostics` and `import_fragments` are released afterwards.
        {
            let mut ctx = LowerContext {
                imports: import_fragments,
                module_prefix: options.module_prefix,
                bindings: options.bindings(),
                bare_import_renames: HashMap::new(),
                remote_locals,
                remote_fn_locals,
                binding_types: HashMap::new(),
                enums: HashMap::new(),
                type_aliases: HashMap::new(),
                option_bindings: HashSet::new(),
                traits: BTreeMap::new(),
                user_structs: HashSet::new(),
                dyn_locals: HashMap::new(),
                dyn_fn_params: HashMap::new(),
                from_conversions: HashMap::new(),
                into_params: HashSet::new(),
                return_into: false,
                in_unsafe: false,
                assoc_bindings: HashMap::new(),
                vtables: Vec::new(),
                lints: options.lints,
                diagnostics,
                try_hoists: Vec::new(),
                try_tmp_counter: 0,
                meta_markers: super::meta_markers::collect_module_meta_markers(items),
            };
            super::types::collect_type_aliases(items, &mut ctx.type_aliases)?;
            super::traits::collect_traits(items, &mut ctx.traits)?;
            if let Some(catalog) = options.trait_catalog {
                super::traits::seed_traits_from_imports(
                    items,
                    catalog,
                    &mut ctx.traits,
                    options.module_prefix,
                    options.bindings(),
                )?;
            }
            super::collect_user_structs(items, &mut ctx.user_structs);
            super::traits::collect_dyn_fn_params(items, &mut ctx.dyn_fn_params);

            for bench_fn in bench_fns_at_level {
                let bench_args = bench_fn
                    .attrs
                    .iter()
                    .find_map(attrs::parse_factorio_bench_attribute)
                    .unwrap_or(attrs::BenchAttributeArgs { iterations: 1 });

                let lowered = lower_function(bench_fn, &mut ctx)?;
                let report_name = if path_prefix.is_empty() {
                    lowered.name.clone()
                } else {
                    format!("{path_prefix}::{}", lowered.name)
                };
                let lua_name = unique_lua_name(&lowered.name, seen_lua_names);
                benches.push(FactorioBench {
                    name: report_name,
                    lua_name,
                    iterations: bench_args.iterations,
                    function: lowered,
                });
            }
        }
    }

    // Recurse into nested mods and stage-bang macros regardless of cfg(test).
    for item in items {
        match item {
            Item::Macro(item_macro)
                if attrs::is_factorio_stage_bang(&item_macro.mac.path).is_some() =>
            {
                let nested = parse_macro_items(item_macro.mac.tokens.clone())?;
                collect_from_items(
                    &nested,
                    // _source_dir,
                    // _source_path,
                    path_prefix,
                    options,
                    diagnostics,
                    benches,
                    helpers,
                    vtables,
                    seen_lua_names,
                    import_fragments,
                    parent_imports,
                )?;
            }
            Item::Mod(item_mod) => {
                let mod_name = item_mod.ident.to_string();
                let nested_prefix = if path_prefix.is_empty() {
                    mod_name.clone()
                } else {
                    format!("{path_prefix}::{mod_name}")
                };
                if let Some((_, nested)) = &item_mod.content {
                    collect_from_items(
                        nested,
                        // _source_dir,
                        // _source_path,
                        &nested_prefix,
                        options,
                        diagnostics,
                        benches,
                        helpers,
                        vtables,
                        seen_lua_names,
                        import_fragments,
                        parent_imports,
                    )?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Lower sibling items that bench fns may call (structs, free fns, impls, ...).
///
/// Excludes bench fns themselves, `#[test]` fns, event handlers, and
/// `#[cfg(test)]` modules.
fn lower_parent_support_items(
    parent_items: &[Item],
    options: &ParseOptions<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> FrontendResult<(
    Vec<factorio_ir::statement::Statement>,
    Vec<factorio_ir::module::VTable>,
    Vec<factorio_ir::module::ModuleImport>,
)> {
    let support: Vec<Item> = parent_items
        .iter()
        .filter(|item| include_parent_item_for_benches(item))
        .cloned()
        .collect();
    if support.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }

    let module = super::lower_items(
        &support,
        "factorio_rs_benches",
        factorio_ir::stage::Stage::Control,
        options.module_prefix,
        options.bindings(),
        options.lints,
        diagnostics,
        None,
        options.mod_name,
        options.trait_catalog,
    )?;

    let mut helpers = module.body.statements;
    for symbol in module.symbols {
        helpers.push(symbol.statement);
    }
    Ok((helpers, module.vtables, module.imports))
}

fn include_parent_item_for_benches(item: &Item) -> bool {
    match item {
        // `#[cfg(test)]` mods are not Lua-lowerable.
        Item::Mod(item_mod) if attrs::is_cfg_test(&item_mod.attrs) => false,
        // Skip bench fns themselves.
        Item::Fn(function) if attrs::is_bench_fn(&function.attrs) => false,
        // Skip test fns.
        Item::Fn(function) if attrs::is_test_fn(&function.attrs) => false,
        // Skip event handlers (they register themselves separately).
        Item::Fn(function) if super::event_handler::resolve_event_handler(function).is_some() => {
            false
        }
        Item::Struct(_)
        | Item::Enum(_)
        | Item::Impl(_)
        | Item::Trait(_)
        | Item::Fn(_)
        | Item::Const(_)
        | Item::Type(_)
        | Item::Use(_) => true,
        _ => false,
    }
}

fn merge_parent_helpers(
    helpers: &mut Vec<factorio_ir::statement::Statement>,
    parent_helpers: Vec<factorio_ir::statement::Statement>,
) {
    for stmt in parent_helpers {
        let already = match &stmt {
            factorio_ir::statement::Statement::StructDecl(s) => helpers.iter().any(|h| {
                matches!(
                    h,
                    factorio_ir::statement::Statement::StructDecl(existing)
                        if existing.name == s.name
                )
            }),
            factorio_ir::statement::Statement::EnumDecl(e) => helpers.iter().any(|h| {
                matches!(
                    h,
                    factorio_ir::statement::Statement::EnumDecl(existing)
                        if existing.name == e.name
                )
            }),
            factorio_ir::statement::Statement::FunctionDecl(f) => helpers.iter().any(|h| {
                matches!(
                    h,
                    factorio_ir::statement::Statement::FunctionDecl(existing)
                        if existing.name == f.name
                )
            }),
            _ => false,
        };
        if !already {
            helpers.push(stmt);
        }
    }
}

fn merge_parent_vtables(
    vtables: &mut Vec<factorio_ir::module::VTable>,
    parent_vtables: Vec<factorio_ir::module::VTable>,
) {
    for vt in parent_vtables {
        if !vtables.iter().any(|existing| existing.name == vt.name) {
            vtables.push(vt);
        }
    }
}

fn merge_parent_imports(
    imports: &mut Vec<factorio_ir::module::ModuleImport>,
    parent_imports: Vec<factorio_ir::module::ModuleImport>,
) {
    for import in parent_imports {
        if let Some(existing) = imports.iter_mut().find(|existing| {
            existing.module == import.module && existing.factorio_mod == import.factorio_mod
        }) {
            for item in import.items {
                if !existing.items.iter().any(|known| known.local == item.local) {
                    existing.items.push(item);
                }
            }
        } else {
            imports.push(import);
        }
    }
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
    fn discovers_bench_with_default_iterations() {
        let source = r"
            #[factorio_rs::control]
            mod control {
                #[factorio_rs::bench]
                pub fn my_bench() {}
            }
        ";
        let lints = LintConfig::allow_all();
        let options = ParseOptions::new(&lints);
        let mut diagnostics = Vec::new();
        let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
        let suite =
            discover_benches(Path::new("src"), &sources, &options, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(suite.benches.len(), 1);
        assert_eq!(suite.benches[0].name, "control::my_bench");
        assert_eq!(suite.benches[0].iterations, 1);
    }

    #[test]
    fn discovers_bench_with_explicit_iterations() {
        let source = r"
            #[factorio_rs::control]
            mod control {
                #[factorio_rs::bench(iterations = 3)]
                pub fn heavy_bench() {}
            }
        ";
        let lints = LintConfig::allow_all();
        let options = ParseOptions::new(&lints);
        let mut diagnostics = Vec::new();
        let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
        let suite =
            discover_benches(Path::new("src"), &sources, &options, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(suite.benches.len(), 1);
        assert_eq!(suite.benches[0].name, "control::heavy_bench");
        assert_eq!(suite.benches[0].lua_name, "heavy_bench");
        assert_eq!(suite.benches[0].iterations, 3);

        // to_module must include the bench function.
        let module = suite.to_module();
        assert_eq!(module.name, "factorio_rs_benches");
        assert_eq!(module.symbols.len(), 1);
        let sym = &module.symbols[0];
        assert!(
            matches!(
                &sym.statement,
                factorio_ir::statement::Statement::FunctionDecl(f) if f.name == "heavy_bench"
            ),
            "expected heavy_bench in symbols: {:?}",
            module.symbols
        );
    }

    #[test]
    fn discovers_bench_inside_cfg_test_module() {
        let source = r"
            #[cfg(test)]
            mod benches {
                #[factorio_rs::bench(iterations = 5)]
                pub fn tick_bench() {}
            }
        ";
        let lints = LintConfig::allow_all();
        let options = ParseOptions::new(&lints);
        let mut diagnostics = Vec::new();
        let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
        let suite =
            discover_benches(Path::new("src"), &sources, &options, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(suite.benches.len(), 1);
        assert_eq!(suite.benches[0].name, "benches::tick_bench");
        assert_eq!(suite.benches[0].iterations, 5);
    }

    #[test]
    fn discovers_bench_inside_stage_bang_macro() {
        let source = r"
            factorio_rs::control_mod! {
                #[factorio_rs::bench(iterations = 2)]
                pub fn smoke_bench() {}
            }
        ";
        let lints = LintConfig::allow_all();
        let options = ParseOptions::new(&lints);
        let mut diagnostics = Vec::new();
        let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
        let suite =
            discover_benches(Path::new("src"), &sources, &options, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(suite.benches.len(), 1);
        assert_eq!(suite.benches[0].name, "smoke_bench");
        assert_eq!(suite.benches[0].iterations, 2);
    }

    #[test]
    fn bench_fn_is_not_included_as_helper() {
        let source = r"
            fn helper() {}
            #[factorio_rs::bench]
            pub fn my_bench() { helper(); }
        ";
        let lints = LintConfig::allow_all();
        let options = ParseOptions::new(&lints);
        let mut diagnostics = Vec::new();
        let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
        let suite =
            discover_benches(Path::new("src"), &sources, &options, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(suite.benches.len(), 1);
        // `my_bench` must not appear in helpers.
        assert!(
            !suite.helpers.iter().any(|h| matches!(
                h,
                factorio_ir::statement::Statement::FunctionDecl(f) if f.name == "my_bench"
            )),
            "bench fn must not be copied into helpers: {:?}",
            suite.helpers
        );
        // `helper` must appear in helpers.
        assert!(
            suite.helpers.iter().any(|h| matches!(
                h,
                factorio_ir::statement::Statement::FunctionDecl(f) if f.name == "helper"
            )),
            "expected helper in helpers: {:?}",
            suite.helpers
        );
    }

    #[test]
    fn parse_bench_attribute_default_iterations() {
        use syn::parse_str;
        let Ok(item_fn) = parse_str::<syn::ItemFn>(r"#[factorio_rs::bench] pub fn b() {}") else {
            panic!("failed to parse fn");
        };
        let args = item_fn
            .attrs
            .iter()
            .find_map(crate::lower::attrs::parse_factorio_bench_attribute)
            .expect("bench attribute not found");
        assert_eq!(args.iterations, 1);
    }

    #[test]
    fn parse_bench_attribute_explicit_iterations() {
        use syn::parse_str;
        let Ok(item_fn) =
            parse_str::<syn::ItemFn>(r"#[factorio_rs::bench(iterations = 42)] pub fn b() {}")
        else {
            panic!("failed to parse fn");
        };
        let args = item_fn
            .attrs
            .iter()
            .find_map(crate::lower::attrs::parse_factorio_bench_attribute)
            .expect("bench attribute not found");
        assert_eq!(args.iterations, 42);
    }
}
