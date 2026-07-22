use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use factorio_codegen::{EmitModOptions, emit_mod_tree};
use factorio_frontend::{
    ParseOptions, build_trait_catalog, discover_modules, parse_discovered_module_with_options,
    resolve_project_locales,
};
use factorio_ir::lint::LintConfig;
use wasm_bindgen::prelude::*;

const MOD_NAME: &str = "playground";
const VIRTUAL_SRC: &str = "/virtual/src";

/// Result of lowering a single Rust snippet to Lua.
#[wasm_bindgen(getter_with_clone)]
#[derive(Clone, Debug)]
pub struct TranspileResult {
    /// Whether lowering and codegen succeeded.
    pub ok: bool,
    /// Emitted Lua when `ok` is true.
    pub lua: Option<String>,
    /// Human-readable error when `ok` is false.
    pub message: Option<String>,
}

/// Result of lowering a multi-file virtual crate to a Factorio mod tree.
#[wasm_bindgen(getter_with_clone)]
#[derive(Clone, Debug)]
pub struct TranspileFilesResult {
    /// Whether lowering and codegen succeeded.
    pub ok: bool,
    /// JSON object mapping mod-relative paths to file contents when `ok`.
    pub files_json: Option<String>,
    /// Human-readable error when `ok` is false.
    pub message: Option<String>,
}

/// Lower `source` as a factorio-rs module named `module_name` and emit Lua.
#[wasm_bindgen]
#[must_use]
pub fn transpile(source: &str, module_name: &str) -> TranspileResult {
    let files = serde_json::json!({ format!("{module_name}.rs"): source });
    let result = transpile_files(&files.to_string());
    if !result.ok {
        return TranspileResult {
            ok: false,
            lua: None,
            message: result.message,
        };
    }
    let map: BTreeMap<String, String> =
        serde_json::from_str(&result.files_json.unwrap_or_default()).unwrap_or_default();
    let preferred = format!("lua/{}.lua", module_name.replace('.', "/"));
    let lua = map
        .get(&preferred)
        .cloned()
        .or_else(|| map.get("control.lua").cloned())
        .or_else(|| map.values().find(|v| v.contains("function")).cloned());
    TranspileResult {
        ok: true,
        lua,
        message: None,
    }
}

/// Lower a virtual multi-file crate into a Factorio mod file tree.
///
/// `files_json` maps virtual paths under `src/` to Rust source. On success,
/// `files_json` in the result includes `info.json`, `control.lua`, stage entry
/// scripts, locale `.cfg` files, and module Lua under `lua/`.
#[wasm_bindgen]
#[must_use]
pub fn transpile_files(files_json: &str) -> TranspileFilesResult {
    match transpile_files_inner(files_json) {
        Ok(files) => match serde_json::to_string(&files) {
            Ok(files_json) => TranspileFilesResult {
                ok: true,
                files_json: Some(files_json),
                message: None,
            },
            Err(error) => TranspileFilesResult {
                ok: false,
                files_json: None,
                message: Some(error.to_string()),
            },
        },
        Err(message) => TranspileFilesResult {
            ok: false,
            files_json: None,
            message: Some(message),
        },
    }
}

fn transpile_files_inner(files_json: &str) -> Result<BTreeMap<String, String>, String> {
    let files: BTreeMap<String, String> =
        serde_json::from_str(files_json).map_err(|error| error.to_string())?;
    if files.is_empty() {
        return Err("add at least one Rust file".to_string());
    }

    let source_dir = Path::new(VIRTUAL_SRC);
    let sources: Vec<(PathBuf, String)> = files
        .iter()
        .map(|(rel, source)| {
            let normalized = rel.trim_start_matches('/').trim_start_matches("src/");
            (source_dir.join(normalized), source.clone())
        })
        .collect();

    let catalog = build_trait_catalog(&sources, source_dir).map_err(|error| error.to_string())?;
    let lints = LintConfig::allow_all();
    let options = ParseOptions::new(&lints)
        .with_mod_name(MOD_NAME)
        .with_trait_catalog(&catalog);

    let mut modules = Vec::new();
    for (path, source) in &sources {
        let discovered =
            discover_modules(source_dir, path, source).map_err(|error| error.to_string())?;
        if discovered.is_empty() {
            return Err(format!(
                "no factorio-rs module discovered in `{}` (use paths like control/foo.rs or shared/bar.rs)",
                path.strip_prefix(source_dir)
                    .map_or_else(|_| path.display().to_string(), |p| p.display().to_string(),)
            ));
        }
        for module in discovered {
            let mut diagnostics = Vec::new();
            let ir = parse_discovered_module_with_options(&module, &options, &mut diagnostics)
                .map_err(|error| format!("{} (in `{}`)", error, module.module_name))?;
            let _ = diagnostics;
            modules.push(ir);
        }
    }

    resolve_project_locales(&mut modules).map_err(|error| error.to_string())?;

    emit_mod_tree(&modules, &EmitModOptions::playground(MOD_NAME))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::transpile_files;

    #[test]
    fn transpile_files_emits_mod_packaging() {
        let files = serde_json::json!({
            "control/on_singleplayer_init.rs": r#"
#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    println!("Hello factorio-rs!");
}
"#,
        });
        let result = transpile_files(&files.to_string());
        assert!(result.ok, "{:?}", result.message);
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&result.files_json.expect("files")).expect("json");
        assert!(map.contains_key("control.lua"));
        assert!(map.contains_key("info.json"));
        assert!(map.contains_key("lua/control/on_singleplayer_init.lua"));
        let control = map["control.lua"].as_str().expect("str");
        assert!(
            control.contains("script.on_event(defines.events.on_singleplayer_init"),
            "{control}"
        );
    }

    #[test]
    fn transpile_files_nested_modules() {
        let files = serde_json::json!({
                                    "shared/player.rs": r"
mod health;

pub struct MyPlayer {
    health: u64,
}

impl MyPlayer {
    pub fn new() -> Self {
        Self {
            health: Self::DEFAULT_HEALTH,
        }
    }
}
",
                                    "shared/player/health.rs": r"
use crate::shared::player::MyPlayer;

impl MyPlayer {
    pub const DEFAULT_HEALTH: u64 = 100;

    pub fn get_health(&self) -> u64 {
        self.health
    }

    pub fn set_health(&mut self, health: u64) {
        self.health = health;
    }
}
",
                                    "control/on_init.rs": r"
pub fn on_init() {
    let mut player = crate::shared::player::MyPlayer::new();
    player.set_health(player.get_health() - 1);
}
",
                                    "data/items.rs": r#"
item! {
    widget {
        name = "playground-widget",
        icon = "graphics/icon.png",
        stack_size = 50,
        icon_size = 64,
    }
}

locale! {
    file = "item-names",
    en {
        "item-name" {
            Items::WIDGET = "Playground Widget",
        }
    }
}
"#,
                                });
        let result = transpile_files(&files.to_string());
        assert!(result.ok, "{:?}", result.message);
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&result.files_json.expect("files")).expect("json");
        assert!(map.contains_key("data.lua"));
        assert!(map.contains_key("lua/data/items.lua"));
        assert!(map.contains_key("locale/en/item-names.cfg"));
        let data = map["data.lua"].as_str().expect("str");
        assert!(
            data.contains("require(\"__playground__/lua/data/items\")"),
            "{data}"
        );
        let locale = map["locale/en/item-names.cfg"].as_str().expect("str");
        assert!(
            locale.contains("playground-widget=Playground Widget"),
            "{locale}"
        );
    }

    #[test]
    fn transpile_files_rejects_empty() {
        let result = transpile_files("{}");
        assert!(!result.ok);
    }
}
