use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use factorio_codegen::{EmitModOptions, emit_mod_tree};
use factorio_frontend::{
    ParseOptions, build_trait_catalog, discover_modules, parse_discovered_module_with_options,
    resolve_project_locales,
};
use factorio_ir::{lint::LintConfig, opt::optimize_modules, prune::prune_modules};
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
    let result = transpile_files(&files.to_string(), "debug");
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

#[wasm_bindgen]
#[must_use]
pub fn transpile_files(files_json: &str, profile: &str) -> TranspileFilesResult {
    match transpile_files_inner(files_json, profile) {
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

fn transpile_files_inner(
    files_json: &str,
    profile: &str,
) -> Result<BTreeMap<String, String>, String> {
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

    let is_debug = profile == "debug";
    if !is_debug {
        prune_modules(&mut modules);
        optimize_modules(&mut modules);
    }

    let mut emit = EmitModOptions::playground(MOD_NAME);
    emit.profile = if is_debug { "debug" } else { "release" };
    emit.debug_level = if is_debug { Some(1) } else { None };

    emit_mod_tree(&modules, &emit).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::transpile_files;
    use std::collections::BTreeMap;

    fn fixture_files(entries: &[(&str, &str)]) -> serde_json::Value {
        serde_json::Value::Object(
            entries
                .iter()
                .map(|(path, source)| {
                    (
                        (*path).to_owned(),
                        serde_json::Value::String((*source).to_owned()),
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn transpile_files_emits_mod_packaging() {
        let files = fixture_files(&[(
            "control/on_singleplayer_init.rs",
            r#"
#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    println!("Hello factorio-rs!");
}
"#,
        )]);
        let result = transpile_files(&files.to_string(), "debug");
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
    fn transpile_files_release_runs_optimize() {
        let files = fixture_files(&[(
            "control/pick.rs",
            r"
pub fn pick(c: bool) -> i32 {
    let x = if c { 1 } else { 0 };
    x
}
",
        )]);
        let debug = transpile_files(&files.to_string(), "debug");
        assert!(debug.ok, "{:?}", debug.message);
        let release = transpile_files(&files.to_string(), "release");
        assert!(release.ok, "{:?}", release.message);

        let debug_map: BTreeMap<String, String> =
            serde_json::from_str(&debug.files_json.expect("files")).expect("json");
        let release_map: BTreeMap<String, String> =
            serde_json::from_str(&release.files_json.expect("files")).expect("json");
        let debug_lua = debug_map.get("lua/control/pick.lua").expect("debug lua");
        let release_lua = release_map
            .get("lua/control/pick.lua")
            .expect("release lua");

        assert!(debug_lua.contains("-- Profile: debug"), "{debug_lua}");
        assert!(release_lua.contains("-- Profile: release"), "{release_lua}");
        assert!(
            !release_lua.contains("(function()"),
            "release should hoist statement-context if:\n{release_lua}"
        );
    }

    #[test]
    fn transpile_files_release_simplifies_unwrap_or() {
        let files = fixture_files(&[(
            "control/boot.rs",
            r#"
#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let n = storage.get::<u32>("boots").unwrap_or(0);
    storage.set("boots", n + 1);
    println!("boot count: {}", n + 1);
}
"#,
        )]);
        let release = transpile_files(&files.to_string(), "release");
        assert!(release.ok, "{:?}", release.message);
        let map: BTreeMap<String, String> =
            serde_json::from_str(&release.files_json.expect("files")).expect("json");
        let lua = map.get("lua/control/boot.lua").expect("lua");
        assert!(
            !lua.contains("local n = nil"),
            "unwrap_or should not leave nil init:\n{lua}"
        );
        assert!(
            !lua.contains("local __o"),
            "bind-once temp should be folded away:\n{lua}"
        );
        assert!(
            lua.contains("local n = storage[\"boots\"]")
                || lua.contains("local n = storage['boots']"),
            "{lua}"
        );
        assert!(lua.contains("if n == nil then"), "{lua}");
        assert!(
            lua.contains("n = n + 1"),
            "repeated n + 1 should mutate local once:\n{lua}"
        );
        assert_eq!(
            lua.matches("n + 1").count(),
            1,
            "n + 1 should appear once after peephole:\n{lua}"
        );
    }

    #[test]
    fn transpile_files_release_hoists_unwrap_or_in_binop() {
        let files = fixture_files(&[(
            "control/counter.rs",
            r#"
#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    storage.set("n", storage.get::<u32>("n").unwrap_or(0) + 1);
}
"#,
        )]);
        let release = transpile_files(&files.to_string(), "release");
        assert!(release.ok, "{:?}", release.message);
        let map: BTreeMap<String, String> =
            serde_json::from_str(&release.files_json.expect("files")).expect("json");
        let lua = map.get("lua/control/counter.lua").expect("lua");
        assert!(
            !lua.contains("(function()"),
            "unwrap_or in binop should not stay an IIFE:\n{lua}"
        );
        assert!(
            lua.contains("== nil") || lua.contains("~= nil"),
            "expected nil check:\n{lua}"
        );
    }

    #[test]
    fn transpile_files_release_fuses_ok_or_question() {
        let files = fixture_files(&[(
            "control/place.rs",
            r#"
fn bump() -> Result<u32, String> {
    let n = storage.get::<u32>("n").ok_or("missing")?;
    storage.set("n", n + 1);
    Ok(n)
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let _ = bump();
}
"#,
        )]);
        let release = transpile_files(&files.to_string(), "release");
        assert!(release.ok, "{:?}", release.message);
        let map: BTreeMap<String, String> =
            serde_json::from_str(&release.files_json.expect("files")).expect("json");
        let lua = map.get("lua/control/place.lua").expect("lua");
        assert!(
            lua.contains("== nil") && lua.contains("err = \"missing\""),
            "expected fused nil -> return err:\n{lua}"
        );
        assert!(
            !lua.contains(".ok"),
            "ok_or? should not load `.ok` from a Result wrapper:\n{lua}"
        );
        // Prefer the folded form (`local n = storage[...]`); accept a leftover
        // `__try_` temp only if it is still bound into `n`.
        assert!(
            lua.contains("local n = storage[\"n\"]")
                || lua.contains("local n = storage['n']")
                || lua.contains("local n = __try_")
                || lua.contains("local n=__try_"),
            "{lua}"
        );
    }

    #[test]
    fn transpile_files_release_calls_enum_tick_method() {
        let files = fixture_files(&[
            (
                "shared/phase.rs",
                r"
pub enum Phase {
    Idle,
    Mining { ticks: i64 },
}

impl Phase {
    pub fn tick(self) -> Phase {
        match self {
            Phase::Idle => Phase::Mining { ticks: 0 },
            Phase::Mining { ticks } => Phase::Mining { ticks: ticks + 1 },
        }
    }
}
",
            ),
            (
                "control/tick.rs",
                r#"
use crate::shared::phase::Phase;

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let mut phase = storage.get::<Phase>("phase").unwrap_or(Phase::Idle);
    phase = phase.tick();
    storage.set("phase", phase);
}
"#,
            ),
        ]);
        let release = transpile_files(&files.to_string(), "release");
        assert!(release.ok, "{:?}", release.message);
        let map: BTreeMap<String, String> =
            serde_json::from_str(&release.files_json.expect("files")).expect("json");
        let lua = map.get("lua/control/tick.lua").expect("lua");
        assert!(
            lua.contains("Phase.tick(phase)") || lua.contains("phase:tick()"),
            "enum method must be invoked with self, got:\n{lua}"
        );
        assert!(
            !lua.contains("phase = phase.tick\n") && !lua.contains("phase = phase.tick\r"),
            "must not emit property read for tick():\n{lua}"
        );
    }

    #[test]
    fn transpile_files_release_folds_match_in_if_condition() {
        let files = fixture_files(&[(
            "control/tick.rs",
            r#"
pub enum Phase {
    Idle,
    Mining,
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let phase = Phase::Mining;
    if matches!(phase, Phase::Mining) {
        println!("mining started");
    }
}
"#,
        )]);
        let release = transpile_files(&files.to_string(), "release");
        assert!(release.ok, "{:?}", release.message);
        let map: BTreeMap<String, String> =
            serde_json::from_str(&release.files_json.expect("files")).expect("json");
        let lua = map.get("lua/control/tick.lua").expect("lua");
        assert!(
            !lua.contains("(function()"),
            "match IIFE in if-condition should collapse:\n{lua}"
        );
        assert!(
            !lua.contains("__match_"),
            "match temp should be folded away:\n{lua}"
        );
        assert!(
            lua.contains("phase.tag == \"Mining\"") || lua.contains("phase.tag == 'Mining'"),
            "{lua}"
        );
    }

    #[test]
    fn transpile_files_release_folds_enum_tag_match() {
        let files = fixture_files(&[(
            "control/phase.rs",
            r#"
pub enum Phase {
    Idle,
    Running,
}

fn is_running(phase: Phase) -> bool {
    match phase {
        Phase::Running => true,
        _ => false,
    }
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let phase = Phase::Running;
    if is_running(phase) {
        println!("running");
    }
}
"#,
        )]);
        let release = transpile_files(&files.to_string(), "release");
        assert!(release.ok, "{:?}", release.message);
        let map: BTreeMap<String, String> =
            serde_json::from_str(&release.files_json.expect("files")).expect("json");
        let lua = map.get("lua/control/phase.lua").expect("lua");
        assert!(
            !lua.contains("__match_"),
            "match temp should be folded away:\n{lua}"
        );
        assert!(
            lua.contains("return phase.tag == \"Running\"")
                || lua.contains("return phase.tag == 'Running'"),
            "{lua}"
        );
    }

    #[test]
    fn transpile_files_nested_modules() {
        let files = fixture_files(&[
            (
                "shared/player.rs",
                r"
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
            ),
            (
                "shared/player/health.rs",
                r"
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
            ),
            (
                "control/on_init.rs",
                r"
pub fn on_init() {
    let mut player = crate::shared::player::MyPlayer::new();
    player.set_health(player.get_health() - 1);
}
",
            ),
            (
                "data/items.rs",
                r#"
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
            ),
        ]);
        let result = transpile_files(&files.to_string(), "debug");
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
        let result = transpile_files("{}", "debug");
        assert!(!result.ok);
    }
}
