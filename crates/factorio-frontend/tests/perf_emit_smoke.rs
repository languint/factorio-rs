#![allow(clippy::expect_used, clippy::unwrap_used)]
mod common;

use common::must_ok_parse;
use factorio_codegen::LuaGenerator;
use factorio_frontend::parse_module;

#[test]
fn option_unwrap_or_emits_no_iife() {
    let module = must_ok_parse(parse_module(
        r"
pub fn hot(x: Option<i32>) -> i32 {
    x.unwrap_or(0)
}
",
        "control.hot",
    ));
    let lua = LuaGenerator::new().generate_module(&module).expect("lua");
    assert!(
        !lua.contains("function()"),
        "unwrap_or should not emit IIFE, got:\n{lua}"
    );
    assert!(lua.contains("if "), "expected statement if:\n{lua}");
}

#[test]
fn value_match_emits_no_iife() {
    let module = must_ok_parse(parse_module(
        r"
pub fn classify(flag: bool) -> i32 {
    match flag {
        true => 1,
        false => 0,
    }
}
",
        "control.match_hot",
    ));
    let lua = LuaGenerator::new().generate_module(&module).expect("lua");
    assert!(
        !lua.contains("function()"),
        "value match should not emit IIFE, got:\n{lua}"
    );
}
