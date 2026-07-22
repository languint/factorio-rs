#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::literal_string_with_formatting_args
)]

fn ok(label: &str, files: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let result = factorio_playground::transpile_files(&files.to_string());
    assert!(result.ok, "{label}: {:?}", result.message);
    serde_json::from_str(result.files_json.as_ref().expect("files")).expect("json")
}

#[test]
fn screen_gui() {
    let map = ok(
        "screen-gui",
        &serde_json::json!({
            "control/gui.rs": r#"
use factorio_rs::{
    factorio_api::{classes::LuaGuiElementAddParams, IndexOrName},
    prelude::*,
};

#[factorio_rs::event(OnPlayerCreated)]
pub fn on_player_created(event: OnPlayerCreatedEvent) {
    if let Some(player) = game.get_player(IndexOrName::Index(event.player_index)) {
        let frame = player.gui().screen().add(LuaGuiElementAddParams {
            r#type: GuiElementType::Frame,
            name: Some("playground_root".into()),
            caption: Some("Playground".into()),
            ..Default::default()
        });

        let label = frame.add(LuaGuiElementAddParams {
            r#type: GuiElementType::Label,
            caption: Some("Hello from factorio-rs".into()),
            ..Default::default()
        });

        frame.style().set_width(280);
        label.style().set_padding(8);
    }
}
"#,
        }),
    );
    assert!(
        map["control.lua"]
            .as_str()
            .unwrap()
            .contains("on_player_created")
    );
    assert!(
        map["lua/control/gui.lua"]
            .as_str()
            .unwrap()
            .contains("GuiElementType")
            || map["lua/control/gui.lua"]
                .as_str()
                .unwrap()
                .contains("frame")
    );
}

#[test]
fn filtered_event() {
    let map = ok(
        "filtered-event",
        &serde_json::json!({
            "control/on_built_entity.rs": r#"
#[factorio_rs::event(filter = OnBuiltEntityFilter::name("inserter"))]
pub fn on_built_entity(event: OnBuiltEntityEvent) {
    let x = event.entity.position().x;
    let y = event.entity.position().y;
    println!("inserter built at: ({x},{y})");
}
"#,
        }),
    );
    assert!(
        map["control.lua"]
            .as_str()
            .unwrap()
            .contains("script.on_event")
    );
}

#[test]
fn remote_api() {
    let map = ok(
        "remote-api",
        &serde_json::json!({
            "control/api.rs": r#"
#[factorio_rs::export]
pub fn greet(name: &str) -> String {
    format!("hello, {name}")
}

#[factorio_rs::export(interface = "playground-admin")]
pub fn ping() {
    println!("pong");
}
"#,
        }),
    );
    let control = map["control.lua"].as_str().unwrap();
    assert!(control.contains("remote.add_interface"), "{control}");
}

#[test]
fn storage() {
    let map = ok(
        "storage",
        &serde_json::json!({
            "control/boot.rs": r#"
#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let n = storage.get::<u32>("boots").unwrap_or(0);
    storage.set("boots", n + 1);
    println!("boot count: {}", n + 1);
}
"#,
        }),
    );
    assert!(
        map["lua/control/boot.lua"]
            .as_str()
            .unwrap()
            .contains("storage[")
    );
}

#[test]
fn enum_phase() {
    ok(
        "enum-phase",
        &serde_json::json!({
                                            "shared/phase.rs": r"
pub enum Phase {
    Idle,
    Mining { ticks: i64 },
    Done,
}

impl Phase {
    pub fn tick(self) -> Phase {
        match self {
            Phase::Idle => Phase::Mining { ticks: 0 },
            Phase::Mining { ticks } if ticks + 1 >= 60 => Phase::Done,
            Phase::Mining { ticks } => Phase::Mining { ticks: ticks + 1 },
            Phase::Done => Phase::Done,
        }
    }
}
",
                                            "control/tick.rs": r#"
use crate::shared::phase::Phase;

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let mut phase = storage
        .get::<Phase>("phase")
        .unwrap_or(Phase::Idle);
    phase = phase.tick();
    storage.set("phase", phase);
    if matches!(phase, Phase::Mining { .. }) {
        println!("mining started");
    }
}
"#,
                                        }),
    );
}

#[test]
fn traits_dyn() {
    let map = ok(
        "traits-dyn",
        &serde_json::json!({
            "shared/alert.rs": r#"
pub trait Alert {
    fn title(&self) -> &'static str;
    fn priority(&self) -> i64;

    fn announce(&self) {
        println!("[alert p{}] {}", self.priority(), self.title());
    }
}
"#,
            "control/alerts.rs": r#"
use crate::shared::alert::Alert;

struct BeltJam {
    lane: &'static str,
}

impl Alert for BeltJam {
    fn title(&self) -> &'static str {
        self.lane
    }

    fn priority(&self) -> i64 {
        40
    }

    fn announce(&self) {
        println!("[belt jammed] {}", self.lane);
    }
}

fn shout(a: &dyn Alert) {
    a.announce();
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let jam = BeltJam {
        lane: "iron-plate",
    };
    jam.announce();
    shout(&jam);
}
"#,
        }),
    );
    let lua = map["lua/control/alerts.lua"].as_str().unwrap();
    assert!(lua.contains("__vt_Alert") || lua.contains("_vt"), "{lua}");
}

#[test]
fn result_try() {
    ok(
        "result-try",
        &serde_json::json!({
            "control/place.rs": r#"
pub fn place(name: &str) -> Result<i32, &'static str> {
    let n = Some(1).ok_or("missing")?;
    if name.is_empty() {
        return Err("empty name");
    }
    Ok(n + 1)
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    match place("inserter") {
        Ok(n) => println!("ok count={n}"),
        Err(e) => println!("err: {e}"),
    }
}
"#,
        }),
    );
}

#[test]
fn prototypes() {
    let map = ok(
        "prototypes",
        &serde_json::json!({
            "data/prototypes.rs": r#"
item! {
    widget {
        name = "playground-widget",
        icon = "graphics/icon.png",
        stack_size = 50,
        icon_size = 64,
    }
}

recipe! {
    craft_widget {
        name = "playground-widget",
        energy_required = 1.0,
        ingredients = [
            { name = "iron-plate", amount = 2 },
            { name = "copper-plate", amount = 1 },
        ],
        results = [
            { name = Items::WIDGET, amount = 1 },
        ],
        category = "crafting",
        enabled = true,
    }
}

locale! {
    file = "names",
    en {
        "item-name" {
            Items::WIDGET = "Playground Widget",
        }
        "recipe-name" {
            Recipes::CRAFT_WIDGET = "Craft Widget",
        }
    }
}
"#,
        }),
    );
    assert!(map.contains_key("data.lua"));
    assert!(map.contains_key("locale/en/names.cfg"));
}

#[test]
fn iterators() {
    ok(
        "iterators",
        &serde_json::json!({
            "control/scores.rs": r#"
pub fn top_half(scores: Vec<i64>) -> Vec<i64> {
    scores.iter().filter(|s| *s >= 50).collect::<Vec<_>>()
}

pub fn indices(n: i64) -> Vec<i64> {
    (0..n).map(|i| i + 1).collect::<Vec<_>>()
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let scores = (0..5).map(|i| i * 20).collect::<Vec<_>>();
    let xs = top_half(scores);
    let ys = indices(3);
    println!("kept={} first={}", xs.len(), ys[0]);
}
"#,
        }),
    );
}

#[test]
fn settings() {
    let map = ok(
        "settings",
        &serde_json::json!({
            "settings/mod.rs": r#"
mod_settings! {
    prefix = "pg",
    startup {
        enabled: bool = true,
        speed: f64 = 1.0,
    }
    runtime_global {
        announce: bool = true,
    }
}

locale! {
    file = "settings",
    en {
        mod_setting_name {
            Settings::ENABLED = "Enable playground features",
            Settings::SPEED = "Simulation speed",
            Settings::ANNOUNCE = "Announce on tick",
        }
    }
}
"#,
            "control/boot.rs": r#"
use crate::settings::Settings;

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    if settings.startup.get_bool(Settings::ENABLED) {
        let speed = settings.startup.get::<f64>(Settings::SPEED);
        println!("playground enabled at speed={speed:?}");
    }
}
"#,
        }),
    );
    assert!(map.contains_key("settings.lua"));
    assert!(map.contains_key("locale/en/settings.cfg"));
}

#[test]
fn full_mod() {
    let map = ok(
        "full-mod",
        &serde_json::json!({
                                            "settings/mod.rs": r#"
mod_settings! {
    prefix = "pg",
    startup {
        debug: bool = false,
    }
}
"#,
                                            "data/prototypes.rs": r#"
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
                                            "shared/phase.rs": r"
pub enum Phase {
    Idle,
    Running { ticks: i64 },
}

impl Phase {
    pub fn tick(self) -> Phase {
        match self {
            Phase::Idle => Phase::Running { ticks: 0 },
            Phase::Running { ticks } => Phase::Running { ticks: ticks + 1 },
        }
    }
}
",
                                            "control/boot.rs": r#"
use crate::shared::phase::Phase;

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let mut phase = Phase::Idle;
    phase = phase.tick();
    let boots = storage.get::<u32>("boots").unwrap_or(0);
    storage.set("boots", boots + 1);
    storage.set("phase", phase);
    println!(
        "boot={} running={}",
        boots + 1,
        matches!(phase, Phase::Running { .. })
    );
}
"#,
                                            "control/api.rs": r#"
#[factorio_rs::export]
pub fn status() -> String {
    let boots = storage.get::<u32>("boots").unwrap_or(0);
    format!("boots={boots}")
}
"#,
                                        }),
    );
    assert!(map.contains_key("control.lua"));
    assert!(map.contains_key("data.lua"));
    assert!(map.contains_key("settings.lua"));
    assert!(map.contains_key("info.json"));
    assert!(map.contains_key("locale/en/item-names.cfg"));
    assert!(
        map["control.lua"]
            .as_str()
            .unwrap()
            .contains("remote.add_interface")
    );
}
