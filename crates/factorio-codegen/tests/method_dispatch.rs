#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::missing_const_for_fn
)]

mod common;

use common::{assert_lua_fragment_parses, must_ok};
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::{Expression, MethodDispatch},
    function::Function,
    literal::Literal,
    module::{Module, Symbol},
    scope::Scope,
    stage::Stage,
    statement::Statement,
};

fn id(name: &str) -> Expression {
    Expression::Identifier(name.to_string())
}

fn lit_str(s: &str) -> Expression {
    Expression::Literal(Literal::String(s.to_string()))
}

fn lit_int(n: i64) -> Expression {
    Expression::Literal(Literal::Int(n))
}

fn lit_nil() -> Expression {
    Expression::Literal(Literal::Nil)
}

fn method(receiver: Expression, method: &str, args: Vec<Expression>) -> Expression {
    Expression::method_call(receiver, method, args)
}

fn method_with(
    receiver: Expression,
    method: &str,
    args: Vec<Expression>,
    dispatch: MethodDispatch,
) -> Expression {
    Expression::method_call_with(receiver, method, args, dispatch)
}

fn emit(expr: &Expression) -> String {
    let lua = LuaGenerator::new().generate_expression(expr);
    assert_lua_fragment_parses(&lua);
    lua
}

#[test]
fn storage_get_and_set() {
    assert_eq!(
        emit(&method(id("storage"), "get", vec![lit_str("k")])),
        "storage[\"k\"]"
    );
    assert_eq!(
        emit(&method(
            id("storage"),
            "set",
            vec![lit_str("k"), lit_int(1)]
        )),
        "storage[\"k\"] = 1"
    );
}

#[test]
fn settings_get_variants() {
    let startup = Expression::FieldAccess {
        base: Box::new(id("settings")),
        field: "startup".to_string(),
    };
    assert_eq!(
        emit(&method(startup.clone(), "get", vec![lit_str("flag")])),
        "settings.startup[\"flag\"].value"
    );
    assert_eq!(
        emit(&method(startup.clone(), "get_bool", vec![lit_str("flag")])),
        "settings.startup[\"flag\"].value"
    );
    assert_eq!(
        emit(&method(startup.clone(), "get_int", vec![lit_str("flag")])),
        "settings.startup[\"flag\"].value"
    );
    assert_eq!(
        emit(&method(
            startup.clone(),
            "get_double",
            vec![lit_str("flag")]
        )),
        "settings.startup[\"flag\"].value"
    );
    assert_eq!(
        emit(&method(
            startup.clone(),
            "get_string",
            vec![lit_str("flag")]
        )),
        "settings.startup[\"flag\"].value"
    );
    assert_eq!(
        emit(&method(startup, "setting", vec![lit_str("flag")])),
        "settings.startup[\"flag\"]"
    );
}

#[test]
fn collection_helpers() {
    assert_eq!(emit(&method(id("xs"), "len", vec![])), "#xs");
    assert_eq!(
        emit(&method(id("xs"), "push", vec![lit_int(1)])),
        "table.insert(xs, 1)"
    );
    assert_eq!(emit(&method(id("xs"), "is_empty", vec![])), "#xs == 0");
}

#[test]
fn push_as_statement_uses_append_assign() {
    let module = Module {
        name: "m".to_string(),
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
                name: "f".to_string(),
                params: vec![],
                body: Block {
                    statements: vec![Statement::Expr(Expression::method_call(
                        id("xs"),
                        "push",
                        vec![lit_int(1)],
                    ))],
                },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
                inline: false,
            }),
        }],
    };
    let output = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(
        output.contains("xs[#xs + 1] = 1"),
        "statement push should append, got:\n{output}"
    );
    assert!(
        !output.contains("table.insert"),
        "statement push should not use table.insert:\n{output}"
    );
}

#[test]
fn zero_arg_property_vs_call() {
    assert_eq!(
        emit(&method(id("entity"), "surface", vec![])),
        "entity.surface"
    );
    assert_eq!(emit(&method(id("elem"), "clear", vec![])), "elem.clear()");
    assert_eq!(emit(&method(id("entity"), "die", vec![])), "entity.die()");

    assert_eq!(
        emit(&method(id("entity"), "die", vec![lit_nil(), lit_nil()])),
        "entity.die()"
    );
}

#[test]
fn colon_vs_dot_for_user_methods() {
    assert_eq!(
        emit(&method(id("w"), "caption", vec![lit_str("hi")])),
        "w:caption(\"hi\")"
    );
    // Factorio attribute reads use `.` with no call (`health`, not `get_health`).
    assert_eq!(
        emit(&method(id("entity"), "health", vec![])),
        "entity.health"
    );
    // Unknown zero-arg names are user metatable methods -> `:name()`.
    assert_eq!(
        emit(&method(id("entity"), "get_health", vec![])),
        "entity:get_health()"
    );
    assert_eq!(
        emit(&method(
            id("entity"),
            "set_filter",
            vec![lit_int(1), lit_str("iron-ore")]
        )),
        "entity.set_filter(1, \"iron-ore\")"
    );
}

#[test]
fn trailing_nil_elision_and_attribute_setters() {
    assert_eq!(
        emit(&method(
            id("f"),
            "call",
            vec![lit_int(1), lit_nil(), lit_nil()]
        )),
        "f.call(1)"
    );
    assert_eq!(
        emit(&method(id("elem"), "set_caption", vec![lit_str("Hello")])),
        "elem.caption = \"Hello\""
    );
}

#[test]
fn typed_dispatch_overrides_name_heuristics() {
    assert_eq!(
        emit(&method_with(
            id("bag"),
            "clear",
            vec![],
            MethodDispatch::Colon
        )),
        "bag:clear()"
    );
    assert_eq!(
        emit(&method_with(id("w"), "name", vec![], MethodDispatch::Colon)),
        "w:name()"
    );
    assert_eq!(
        emit(&method_with(
            id("entity"),
            "health",
            vec![],
            MethodDispatch::Factorio
        )),
        "entity.health"
    );
    assert_eq!(
        emit(&method_with(
            id("entity"),
            "die",
            vec![],
            MethodDispatch::Factorio
        )),
        "entity.die()"
    );
    assert_eq!(
        emit(&method_with(
            id("data"),
            "extend",
            vec![Expression::Array { elements: vec![] }],
            MethodDispatch::Colon
        )),
        "data:extend({  })"
    );
}

#[test]
fn method_dispatch_table_covers_rewrites() {
    let cases: &[(&str, Expression, &str)] = &[
        (
            "storage_get",
            method(id("storage"), "get", vec![lit_str("a")]),
            "storage[\"a\"]",
        ),
        (
            "generic_get",
            method(id("t"), "get", vec![lit_str("a")]),
            "t.get(\"a\")",
        ),
        (
            "settings_get_tagged",
            method_with(
                id("t"),
                "get",
                vec![lit_str("a")],
                MethodDispatch::SettingsGet,
            ),
            "t[\"a\"].value",
        ),
        ("len", method(id("t"), "len", vec![]), "#t"),
        (
            "push",
            method(id("t"), "push", vec![lit_int(2)]),
            "table.insert(t, 2)",
        ),
        ("is_empty", method(id("t"), "is_empty", vec![]), "#t == 0"),
        ("clear", method(id("e"), "clear", vec![]), "e.clear()"),
        (
            "new_colon",
            method(id("T"), "new", vec![lit_int(1)]),
            "T:new(1)",
        ),
        ("destroy", method(id("e"), "destroy", vec![]), "e.destroy()"),
    ];

    for (name, expr, expected) in cases {
        let lua = emit(expr);
        assert_eq!(lua, *expected, "case {name}");
    }
}
