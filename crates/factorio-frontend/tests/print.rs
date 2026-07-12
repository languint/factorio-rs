mod common;

use common::{must_ok, must_ok_parse};
use factorio_codegen::LuaGenerator;
use factorio_frontend::parse_module;

#[test]
fn parses_println_with_inline_format_capture() {
    let source = r#"
pub fn on_init() {
    let health = 99;
    println!("my_player health: {health}");
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains(r#"game.print("my_player health: " .. health)"#));
}

#[test]
fn parses_println_with_format_argument() {
    let source = r#"
pub fn on_init() {
    let health = 99;
    println!("health: {}", health);
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains(r#"game.print("health: " .. health)"#));
}

#[test]
fn parses_println_with_literal_only() {
    let source = r#"
pub fn on_init() {
    println!("hello");
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains(r#"game.print("hello")"#));
}

#[test]
fn parses_println_with_multiple_format_arguments() {
    let source = r#"
pub fn on_init() {
    let health = 99;
    let name = "player";
    println!("{} has {} health", name, health);
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains(r#"game.print(name .. " has " .. health .. " health")"#));
}

#[test]
fn parses_format_with_arguments() {
    let source = r#"
pub fn message(name: &str, health: i64) -> String {
    format!("{} has {} health", name, health)
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains(r#"return name .. " has " .. health .. " health""#));
    assert!(!lua.contains("game.print"));
}

#[test]
fn parses_format_with_named_capture() {
    let source = r#"
pub fn message() -> String {
    let health = 10;
    format!("hp={health}")
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains(r#"return "hp=" .. health"#));
}

#[test]
fn parses_format_literal_only() {
    let source = r#"
pub fn message() -> String {
    format!("hello")
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains(r#"return "hello""#));
}

#[test]
fn parses_println_debug_format_userdata_via_tostring() {
    let source = r#"
pub fn on_init(entity: LuaEntity) {
    println!("built {:?}", entity);
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        lua.contains(r#"game.print("built " .. tostring(entity))"#),
        "unexpected lua:\n{lua}"
    );
    assert!(
        !lua.contains("table_to_json") && !lua.contains(r#"type(entity)"#),
        "userdata Debug should not use runtime type check:\n{lua}"
    );
}

#[test]
fn parses_println_debug_format_event_field_via_tostring() {
    let source = r#"
pub fn on_built(event: OnBuiltEntityEvent) {
    println!("built {:?}", event.entity);
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        lua.contains("tostring(event.entity)"),
        "unexpected lua:\n{lua}"
    );
    assert!(
        !lua.contains("table_to_json"),
        "LuaEntity field should not use table_to_json:\n{lua}"
    );
}

#[test]
fn parses_format_named_debug_capture_unknown_via_tostring() {
    let source = r#"
pub fn message(filters: LuaAny) -> String {
    format!("filters={filters:?}")
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        lua.contains("tostring(filters)") && !lua.contains("table_to_json"),
        "unexpected lua:\n{lua}"
    );
}

#[test]
fn parses_format_pretty_debug_table_via_table_to_json() {
    let source = r#"
pub fn message(data: OnBuiltEntityEvent) -> String {
    format!("{:#?}", data)
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        lua.contains("helpers.table_to_json(data)") && !lua.contains("tostring(data)"),
        "unexpected lua:\n{lua}"
    );
}

#[cfg(feature = "tracing")]
#[test]
fn lowers_tracing_info_to_colored_game_print() {
    let source = r#"
pub fn on_init() {
    tracing::info!("ready");
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        lua.contains(
            r#"game.print("[INFO] ready", { color = { r = 0.55, g = 0.85, b = 1, a = 1 } })"#
        ),
        "unexpected lua:\n{lua}"
    );
}

#[cfg(feature = "tracing")]
#[test]
fn lowers_tracing_warn_with_format_args() {
    let source = r#"
pub fn on_init() {
    let name = "iron";
    tracing::warn!("missing {name}");
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        lua.contains(r#"game.print("[WARN] missing " .. name"#),
        "unexpected lua:\n{lua}"
    );
    assert!(lua.contains("color = {"), "expected color settings:\n{lua}");
}

#[cfg(feature = "tracing")]
#[test]
fn lowers_tracing_debug_format_userdata_via_tostring() {
    let source = r#"
pub fn on_init(entity: LuaEntity) {
    tracing::info!("built {:?}", entity);
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(
        lua.contains("tostring(entity)") && !lua.contains("table_to_json"),
        "unexpected lua:\n{lua}"
    );
    assert!(lua.contains("color = {"), "expected color settings:\n{lua}");
}

#[cfg(feature = "tracing")]
#[test]
fn lowers_bare_error_macro_when_tracing_enabled() {
    let source = r#"
pub fn on_init() {
    error!("boom");
}
"#;

    let module = must_ok_parse(parse_module(source, "control.on_init"));
    let lua = must_ok(LuaGenerator::new().generate_module(&module));

    assert!(lua.contains(r#""[ERROR] boom""#), "unexpected lua:\n{lua}");
}
