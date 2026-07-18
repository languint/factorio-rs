#![allow(clippy::unwrap_used)]

mod common;

use factorio_codegen::LuaGenerator;
use factorio_frontend::parse_module;

#[test]
fn assert_macros_lower_to_error_calls() {
    let module = parse_module(
        r"
            pub fn check() {
                assert!(true);
                assert_eq!(1, 1);
                assert_ne!(1, 2);
            }
        ",
        "control",
    )
    .unwrap();

    let lua = LuaGenerator::new().generate_module(&module).unwrap();
    assert!(lua.contains("error("), "expected error() in:\n{lua}");
    assert!(lua.contains("if not true then"), "unexpected lua:\n{lua}");
    assert!(
        lua.contains("__assert_left_") && lua.contains("__assert_right_"),
        "expected assert temps in:\n{lua}"
    );
}

#[test]
fn panic_macro_lowers_to_error() {
    let module = parse_module(
        r#"
            pub fn boom() {
                panic!("kaboom");
            }
        "#,
        "control",
    )
    .unwrap();

    let lua = LuaGenerator::new().generate_module(&module).unwrap();
    assert!(lua.contains(r#"error("kaboom")"#), "unexpected lua:\n{lua}");
}
