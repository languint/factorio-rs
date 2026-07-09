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
