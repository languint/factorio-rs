use std::path::Path;

use factorio_ir::{module::Module, stage::Stage, statement::Statement};
use serde::Serialize;

use crate::{
    cargo_manifest::CargoPackage,
    config::{Config, ModConfig},
    error::{CliError, CliResult},
};

pub use factorio_codegen::{
    EventRegistration, RemoteExport, StageModule, collect_event_registrations,
    collect_remote_exports, collect_stage_module, generate_control_lua, generate_stage_entry_lua,
};

/// A shared-stage (or other requireable) export for dependents / catalogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedExport {
    pub module: String,
    pub function: String,
    pub params: Vec<(String, Option<String>)>,
    /// `#[factorio_rs::inline]` - hot path; dependents use `require`, never remote.
    pub inline: bool,
}

/// A public shared-stage const included in the export catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedConst {
    pub module: String,
    pub name: String,
    pub source_type: Option<String>,
}

/// Collect shared-stage exports for require-based cross-mod APIs.
pub fn collect_shared_exports(module: &Module) -> Vec<SharedExport> {
    if module.stage != Stage::Shared {
        return Vec::new();
    }

    module
        .symbols
        .iter()
        .filter_map(|symbol| {
            let Statement::FunctionDecl(function) = &symbol.statement else {
                return None;
            };
            function.export.as_ref()?;
            Some(SharedExport {
                module: module.name.clone(),
                function: function.name.clone(),
                params: function
                    .params
                    .iter()
                    .map(|param| (param.name.clone(), param.source_type.clone()))
                    .collect(),
                inline: function.inline,
            })
        })
        .collect()
}

/// Collect public shared-stage consts for the export catalog.
pub fn collect_shared_consts(module: &Module) -> Vec<SharedConst> {
    if module.stage != Stage::Shared {
        return Vec::new();
    }

    module
        .symbols
        .iter()
        .filter(|symbol| symbol.scope == factorio_ir::scope::Scope::Public)
        .filter_map(|symbol| {
            let Statement::VariableDecl {
                name, source_type, ..
            } = &symbol.statement
            else {
                return None;
            };
            Some(SharedConst {
                module: module.name.clone(),
                name: name.clone(),
                source_type: source_type.clone(),
            })
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct InfoJson<'a> {
    name: &'a str,
    version: &'a str,
    title: &'a str,
    author: &'a str,
    factorio_version: &'a str,
    dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
}

pub fn generate_info_json(
    package: &CargoPackage,
    mod_config: &ModConfig,
    binding_dependencies: &[String],
) -> CliResult<String> {
    let author = package.author_label();
    let factorio_version = mod_config.factorio_version.as_deref().unwrap_or("2.0");
    let dependencies = merge_dependencies(
        factorio_version,
        &mod_config.dependencies,
        binding_dependencies,
    );
    let info = InfoJson {
        name: &package.name,
        version: &package.version,
        title: mod_config.title.as_deref().unwrap_or(&package.name),
        author: author.as_str(),
        factorio_version,
        dependencies,
        description: mod_config.description.as_deref(),
    };

    serde_json::to_string_pretty(&info).map_err(|source| CliError::InfoJsonSerialize { source })
}

/// Merge Factorio.toml deps (highest priority), binding-crate deps, and a default
/// `base >= {factorio_version}` when `base` is not already listed.
#[must_use]
pub fn merge_dependencies(
    factorio_version: &str,
    from_toml: &[String],
    from_bindings: &[String],
) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::<String>::new();
    let mut out = Vec::new();

    for dep in from_toml.iter().chain(from_bindings.iter()) {
        let key = dependency_mod_name(dep);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        out.push(dep.clone());
    }

    if !seen.contains("base") {
        out.insert(0, format!("base >= {factorio_version}"));
    }

    out
}

/// Extract the Factorio mod name from a dependency string for deduplication.
#[must_use]
pub fn dependency_mod_name(dep: &str) -> String {
    let trimmed = dep.trim();
    let without_prefix = trimmed.strip_prefix("(?)").map_or_else(
        || {
            trimmed
                .strip_prefix(['?', '!', '~', '+'])
                .map_or(trimmed, str::trim_start)
        },
        str::trim_start,
    );

    let token = without_prefix
        .split_whitespace()
        .next()
        .unwrap_or(without_prefix);

    for op in [">=", "<=", "==", "=", ">", "<"] {
        if let Some(idx) = token.find(op) {
            return token[..idx].trim().to_string();
        }
    }

    token.to_string()
}

pub struct StageModules {
    pub entries: Vec<(Stage, StageModule)>,
}

impl StageModules {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, stage: Stage, module: StageModule) {
        self.entries.push((stage, module));
    }

    fn modules_for(&self, stage: Stage) -> Vec<StageModule> {
        self.entries
            .iter()
            .filter(|(s, _)| *s == stage)
            .map(|(_, module)| module.clone())
            .collect()
    }
}

impl Default for StageModules {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn write_mod_manifests(
    output_dir: &Path,
    package: &CargoPackage,
    config: &Config,
    events: &[EventRegistration],
    remote_exports: &[RemoteExport],
    stage_modules: &StageModules,
    profile: &str,
    binding_dependencies: &[String],
) -> CliResult<()> {
    let module_prefix = config.emit.lua_module_prefix.as_deref().unwrap_or("");
    let info_json = generate_info_json(package, &config.r#mod, binding_dependencies)?;
    write_file(output_dir, "info.json", &info_json)?;

    let control_lua = generate_control_lua(
        &package.name,
        events,
        remote_exports,
        module_prefix,
        profile,
    );
    write_file(output_dir, "control.lua", &control_lua)?;

    for stage in Stage::SIDE_EFFECT_STAGES {
        let modules = stage_modules.modules_for(stage);
        if modules.is_empty() {
            continue;
        }
        let Some(entry_file) = stage.entry_file_name() else {
            continue;
        };
        let lua = generate_stage_entry_lua(&package.name, &modules, stage, module_prefix, profile);
        write_file(output_dir, entry_file, &lua)?;
    }

    Ok(())
}

fn write_file(output_dir: &Path, name: &str, contents: &str) -> CliResult<()> {
    let path = output_dir.join(name);
    std::fs::write(&path, contents).map_err(|source| CliError::WriteFile { path, source })
}

#[cfg(test)]
mod tests {
    use factorio_ir::{
        block::Block,
        expression::Expression,
        function::Function,
        literal::Literal,
        module::{Module, Symbol},
        scope::Scope,
        stage::Stage,
        statement::Statement,
    };

    use super::{
        StageModule, StageModules, collect_event_registrations, collect_stage_module,
        dependency_mod_name, generate_control_lua, generate_info_json, generate_stage_entry_lua,
        merge_dependencies, write_mod_manifests,
    };
    use crate::{cargo_manifest::CargoPackage, config::ModConfig};

    fn make_control_module(name: &str, event: &str) -> Module {
        Module {
            name: name.to_string(),
            stage: Stage::Control,
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
            locales: vec![],
            pending_locales: vec![],
            vtables: vec![],
            symbols: vec![Symbol {
                scope: Scope::Public,
                statement: Statement::FunctionDecl(Function {
                    name: event.to_string(),
                    params: vec![],
                    body: Block { statements: vec![] },
                    doc: None,
                    debug: None,
                    event: Some(event.to_string()),
                    event_filter: None,
                    export: None,
                    inline: false,
                }),
            }],
        }
    }

    fn make_stage_module(name: &str, stage: Stage) -> Module {
        Module {
            name: name.to_string(),
            stage,
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
            locales: vec![],
            pending_locales: vec![],
            vtables: vec![],
            symbols: vec![],
        }
    }

    #[test]
    fn generates_control_lua_with_event_handler() {
        let module = make_control_module("control.on_singleplayer_init", "on_singleplayer_init");
        let events = collect_event_registrations(&module);
        let lua = generate_control_lua("hello_world", &events, &[], "", "debug");

        assert!(lua.contains("require(\"__hello_world__/lua/control/on_singleplayer_init\")"));
        assert!(
            lua.contains("script.on_event(defines.events.on_singleplayer_init, function(event)")
        );
        assert!(lua.contains("controlOnSingleplayerInit.on_singleplayer_init(event)"));
        assert!(lua.contains("end)"));
    }

    #[test]
    fn generates_control_lua_with_event_filter() {
        let module = Module {
            name: "control.on_built_entity".to_string(),
            stage: Stage::Control,
            body: Block { statements: vec![] },
            imports: vec![],
            submodules: vec![],
            locales: vec![],
            pending_locales: vec![],
            vtables: vec![],
            symbols: vec![Symbol {
                scope: Scope::Public,
                statement: Statement::FunctionDecl(Function {
                    name: "on_built_entity".to_string(),
                    params: vec![],
                    body: Block { statements: vec![] },
                    doc: None,
                    debug: None,
                    event: Some("on_built_entity".to_string()),
                    event_filter: Some(Expression::Array {
                        elements: vec![Expression::StructLiteral {
                            struct_name: None,
                            fields: vec![
                                (
                                    "filter".to_string(),
                                    Expression::Literal(Literal::String("type".to_string())),
                                ),
                                (
                                    "type".to_string(),
                                    Expression::Literal(Literal::String("inserter".to_string())),
                                ),
                            ],
                        }],
                    }),
                    export: None,
                    inline: false,
                }),
            }],
        };

        let events = collect_event_registrations(&module);
        let lua = generate_control_lua("hello_world", &events, &[], "", "debug");

        assert!(lua.contains("require(\"__hello_world__/lua/control/on_built_entity\")"));
        assert!(lua.contains("script.on_event(defines.events.on_built_entity, function(event)"));
        assert!(lua.contains("end, { { filter = \"type\", type = \"inserter\" } })"));
        assert!(lua.contains("controlOnBuiltEntity.on_built_entity(event)"));
    }

    #[test]
    fn generates_data_lua_for_single_root_module() {
        let modules = vec![StageModule {
            name: "data".to_string(),
            entry_functions: vec!["register".to_string()],
        }];
        let lua = generate_stage_entry_lua("my_mod", &modules, Stage::Data, "", "debug");

        assert!(lua.contains("local data = require(\"__my_mod__/lua/data\")"));
        assert!(lua.contains("data.register()"));
        assert!(!lua.contains("require(\"__my_mod__/lua/data/items\")"));
    }

    #[test]
    fn generates_data_lua_omits_children_when_parent_present() {
        let modules = vec![
            StageModule {
                name: "data".to_string(),
                entry_functions: vec!["register".to_string()],
            },
            StageModule {
                name: "data.items".to_string(),
                entry_functions: vec!["register".to_string()],
            },
        ];
        let lua = generate_stage_entry_lua("my_mod", &modules, Stage::Data, "", "debug");

        assert!(lua.contains("local data = require(\"__my_mod__/lua/data\")"));
        assert!(lua.contains("data.register()"));
        assert!(!lua.contains("data.items"));
    }

    #[test]
    fn generates_data_lua_requires_multiple_root_modules() {
        let modules = vec![
            StageModule {
                name: "data.items".to_string(),
                entry_functions: vec!["register_items".to_string()],
            },
            StageModule {
                name: "data.entities".to_string(),
                entry_functions: vec!["register_entities".to_string()],
            },
        ];
        let lua = generate_stage_entry_lua("my_mod", &modules, Stage::Data, "", "debug");

        assert!(lua.contains("local dataItems = require(\"__my_mod__/lua/data/items\")"));
        assert!(lua.contains("dataItems.register_items()"));
        assert!(lua.contains("local dataEntities = require(\"__my_mod__/lua/data/entities\")"));
        assert!(lua.contains("dataEntities.register_entities()"));
    }

    #[test]
    fn generates_settings_lua() {
        let modules = vec![StageModule {
            name: "settings".to_string(),
            entry_functions: vec!["register".to_string()],
        }];
        let lua = generate_stage_entry_lua("my_mod", &modules, Stage::Settings, "", "debug");

        assert!(lua.contains("local settings = require(\"__my_mod__/lua/settings\")"));
        assert!(lua.contains("settings.register()"));
    }

    #[test]
    fn generates_settings_updates_and_final_fixes_lua() {
        let updates = vec![StageModule {
            name: "settings_updates".to_string(),
            entry_functions: vec!["patch".to_string()],
        }];
        let updates_lua =
            generate_stage_entry_lua("my_mod", &updates, Stage::SettingsUpdates, "", "debug");
        assert!(
            updates_lua
                .contains("local settingsUpdates = require(\"__my_mod__/lua/settings_updates\")")
        );
        assert!(updates_lua.contains("settingsUpdates.patch()"));

        let finals = vec![StageModule {
            name: "settings_final_fixes".to_string(),
            entry_functions: vec!["fixup".to_string()],
        }];
        let finals_lua =
            generate_stage_entry_lua("my_mod", &finals, Stage::SettingsFinalFixes, "", "debug");
        assert!(finals_lua.contains(
            "local settingsFinalFixes = require(\"__my_mod__/lua/settings_final_fixes\")"
        ));
        assert!(finals_lua.contains("settingsFinalFixes.fixup()"));
    }

    #[test]
    fn generates_data_updates_and_final_fixes_lua() {
        let updates = vec![StageModule {
            name: "data_updates".to_string(),
            entry_functions: vec!["patch".to_string()],
        }];
        let updates_lua =
            generate_stage_entry_lua("my_mod", &updates, Stage::DataUpdates, "", "debug");
        assert!(
            updates_lua.contains("local dataUpdates = require(\"__my_mod__/lua/data_updates\")")
        );
        assert!(updates_lua.contains("dataUpdates.patch()"));

        let finals = vec![StageModule {
            name: "data_final_fixes".to_string(),
            entry_functions: vec!["fixup".to_string()],
        }];
        let finals_lua =
            generate_stage_entry_lua("my_mod", &finals, Stage::DataFinalFixes, "", "debug");
        assert!(
            finals_lua
                .contains("local dataFinalFixes = require(\"__my_mod__/lua/data_final_fixes\")")
        );
        assert!(finals_lua.contains("dataFinalFixes.fixup()"));
    }

    #[test]
    fn write_mod_manifests_emits_all_settings_phase_files() {
        use std::fs;

        use tempfile::tempdir;

        use crate::{cargo_manifest::CargoPackage, config::Config};

        let dir = tempdir().expect("tempdir");
        let package = CargoPackage {
            name: "phase_mod".to_string(),
            version: "0.1.0".to_string(),
            authors: Some(vec!["test".to_string()]),
        };
        let config: Config = toml::from_str("").expect("default config");
        let mut stage_modules = StageModules::new();
        stage_modules.push(
            Stage::Settings,
            StageModule {
                name: "settings".to_string(),
                entry_functions: vec!["register".to_string()],
            },
        );
        stage_modules.push(
            Stage::SettingsUpdates,
            StageModule {
                name: "settings_updates".to_string(),
                entry_functions: vec!["patch".to_string()],
            },
        );
        stage_modules.push(
            Stage::SettingsFinalFixes,
            StageModule {
                name: "settings_final_fixes".to_string(),
                entry_functions: vec!["fixup".to_string()],
            },
        );

        write_mod_manifests(
            dir.path(),
            &package,
            &config,
            &[],
            &[],
            &stage_modules,
            "debug",
            &[],
        )
        .expect("write manifests");

        assert!(dir.path().join("settings.lua").is_file());
        assert!(dir.path().join("settings-updates.lua").is_file());
        assert!(dir.path().join("settings-final-fixes.lua").is_file());

        let updates = fs::read_to_string(dir.path().join("settings-updates.lua")).unwrap();
        assert!(updates.contains("settingsUpdates.patch()"));
        let finals = fs::read_to_string(dir.path().join("settings-final-fixes.lua")).unwrap();
        assert!(finals.contains("settingsFinalFixes.fixup()"));
    }

    #[test]
    fn collect_stage_module_returns_exports_for_matching_stage() {
        let data_mod = make_stage_module("data.items", Stage::Data);
        let ctrl_mod = make_stage_module("control", Stage::Control);

        assert_eq!(
            collect_stage_module(&data_mod, Stage::Data),
            Some(StageModule {
                name: "data.items".to_string(),
                entry_functions: vec![],
            })
        );
        assert_eq!(collect_stage_module(&ctrl_mod, Stage::Data), None);
    }

    #[test]
    fn settings_stage_modules_are_not_collected_as_data() {
        let settings_mod = make_stage_module("settings", Stage::Settings);
        assert_eq!(collect_stage_module(&settings_mod, Stage::Data), None);
        assert_eq!(
            collect_stage_module(&settings_mod, Stage::Settings),
            Some(StageModule {
                name: "settings".to_string(),
                entry_functions: vec![],
            })
        );
    }

    #[test]
    fn transpiles_bool_settings_registration() {
        use factorio_codegen::LuaGenerator;
        use factorio_frontend::parse_module;

        let source = r#"
            pub fn register() {
                data.extend([
                    BoolSetting { name: "ms-casual-mode",       setting_type: "startup", default_value: false },
                    BoolSetting { name: "ms-adjacency-enabled", setting_type: "startup", default_value: true  },
                ]);
            }
        "#;

        let module = parse_module(source, "settings").expect("parse settings");
        let lua = LuaGenerator::with_mod_name("mandatory_spaghetti")
            .generate_module(&module)
            .expect("generate settings lua");

        assert!(lua.contains("function settings.register()"));
        assert!(lua.contains("data.extend("));
        assert!(lua.contains("type = \"bool-setting\""));
        assert!(lua.contains("name = \"ms-casual-mode\""));
        assert!(lua.contains("default_value = false"));
        assert!(lua.contains("name = \"ms-adjacency-enabled\""));
        assert!(lua.contains("default_value = true"));

        let entry_lua = generate_stage_entry_lua(
            "mandatory_spaghetti",
            &[StageModule {
                name: "settings".to_string(),
                entry_functions: vec!["register".to_string()],
            }],
            Stage::Settings,
            "",
            "release",
        );
        assert!(entry_lua.contains("-- Profile: release"));
        assert!(entry_lua.contains("settings.register()"));
    }

    #[test]
    fn transpiles_item_prototype_with_packaged_icon() {
        use factorio_codegen::LuaGenerator;
        use factorio_frontend::parse_module;

        let source = r#"
            pub fn register() {
                data.extend([
                    Item {
                        name: "my-mod-widget",
                        icon: "__my_mod__/graphics/icon.png",
                        icon_size: Some(64),
                        stack_size: 50,
                        ..Default::default()
                    },
                ]);
            }
        "#;

        let module = parse_module(source, "data").expect("parse data");
        let lua = LuaGenerator::with_mod_name("my_mod")
            .generate_module(&module)
            .expect("generate data lua");

        assert!(lua.contains("function data.register()"));
        assert!(lua.contains("data.extend("));
        assert!(lua.contains("type = \"item\""));
        assert!(lua.contains("name = \"my-mod-widget\""));
        assert!(lua.contains("icon = \"__my_mod__/graphics/icon.png\""));
        assert!(lua.contains("icon_size = 64"));
        assert!(lua.contains("stack_size = 50"));
        assert!(!lua.contains("subgroup"));
        assert!(!lua.contains("order"));
    }

    #[test]
    fn transpiles_item_macro_with_relative_icon() {
        use factorio_codegen::LuaGenerator;
        use factorio_frontend::{ParseOptions, parse_module_with_options};
        use factorio_ir::lint::LintConfig;

        let source = r#"
            item! {
                widget {
                    name = "my-mod-widget",
                    icon = "graphics/icon.png",
                    stack_size = 50,
                    icon_size = 64,
                    subgroup = "intermediate-product",
                    order = "a[my-mod]-a[widget]",
                }
            }
        "#;

        let lints = LintConfig::allow_all();
        let mut diagnostics = Vec::new();
        let module = parse_module_with_options(
            source,
            "data",
            &ParseOptions::new(&lints).with_mod_name("my_mod"),
            &mut diagnostics,
        )
        .expect("parse data");
        let lua = LuaGenerator::with_mod_name("my_mod")
            .generate_module(&module)
            .expect("generate data lua");

        assert!(lua.contains("function data.register()"));
        assert!(lua.contains("data.extend("));
        assert!(lua.contains("type = \"item\""));
        assert!(lua.contains("name = \"my-mod-widget\""));
        assert!(lua.contains("icon = \"__my_mod__/graphics/icon.png\""));
        assert!(lua.contains("icon_size = 64"));
        assert!(lua.contains("stack_size = 50"));
        assert!(lua.contains("subgroup = \"intermediate-product\""));
    }

    #[test]
    fn dependency_mod_name_strips_prefixes_and_versions() {
        assert_eq!(dependency_mod_name("base >= 2.0"), "base");
        assert_eq!(dependency_mod_name("? space-age"), "space-age");
        assert_eq!(dependency_mod_name("(?) quality"), "quality");
        assert_eq!(dependency_mod_name("! bad-mod"), "bad-mod");
        assert_eq!(dependency_mod_name("~ unordered"), "unordered");
        assert_eq!(dependency_mod_name("+ recommended"), "recommended");
        assert_eq!(dependency_mod_name("bobores>=0.13.1"), "bobores");
        assert_eq!(dependency_mod_name("  flib >= 0.14.0  "), "flib");
    }

    #[test]
    fn merge_dependencies_toml_wins_and_adds_base() {
        let merged = merge_dependencies(
            "2.0",
            &["? space-age".to_string(), "flib >= 0.15".to_string()],
            &["flib >= 0.14".to_string(), "provider >= 0.1.0".to_string()],
        );
        assert_eq!(
            merged,
            vec![
                "base >= 2.0".to_string(),
                "? space-age".to_string(),
                "flib >= 0.15".to_string(),
                "provider >= 0.1.0".to_string(),
            ]
        );
    }

    #[test]
    fn merge_dependencies_respects_explicit_base() {
        let merged = merge_dependencies(
            "2.0",
            &["base >= 2.0.60".to_string()],
            &["provider >= 0.1.0".to_string()],
        );
        assert_eq!(
            merged,
            vec![
                "base >= 2.0.60".to_string(),
                "provider >= 0.1.0".to_string(),
            ]
        );
    }

    #[test]
    fn generate_info_json_includes_toml_dependencies() {
        let package = CargoPackage {
            name: "demo".to_string(),
            version: "0.1.0".to_string(),
            authors: Some(vec!["test".to_string()]),
        };
        let mod_config = ModConfig {
            title: Some("Demo".to_string()),
            description: None,
            factorio_version: Some("2.0".to_string()),
            thumbnail: None,
            assets: Vec::new(),
            dependencies: vec!["! conflict-mod".to_string()],
            emit_api: false,
            api_dir: "api".to_string(),
        };
        let json = generate_info_json(&package, &mod_config, &["flib >= 0.14".to_string()])
            .expect("info.json");
        assert!(json.contains("\"base >= 2.0\""));
        assert!(json.contains("\"! conflict-mod\""));
        assert!(json.contains("\"flib >= 0.14\""));
    }
}
