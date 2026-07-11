use factorio_api_gen::{generate_from_bundled_api, generate_runtime_api, parse_runtime_api};

#[test]
fn bundled_runtime_api_parses() {
    let api = generate_from_bundled_api().expect("bundled runtime-api.json should parse");
    assert!(!api.events.is_empty());
    assert!(api.event_map.contains("event_type_to_name"));
    assert!(api.classes.contains("LuaGameScript"));
    assert!(api.globals.contains("pub static game"));
}

#[test]
fn emits_all_runtime_and_auxiliary_globals() {
    let generated = generate_from_bundled_api().expect("generate");
    let globals = &generated.globals;

    // `global_objects` from runtime-api.json
    for name in [
        "commands",
        "game",
        "helpers",
        "prototypes",
        "rcon",
        "remote",
        "rendering",
        "script",
        "settings",
    ] {
        let needle = format!("pub static {name}");
        assert!(
            globals.contains(&needle) || globals.contains(&format!("pub static {name} ")),
            "missing schema global `{name}`"
        );
    }

    // Auxiliary (not in global_objects)
    assert!(
        globals.contains("pub static storage") || globals.contains("pub static storage "),
        "missing auxiliary global `storage`"
    );
    assert!(
        globals.contains("pub static serpent") || globals.contains("pub static serpent "),
        "missing auxiliary global `serpent`"
    );

    // Global functions from the schema / libraries page
    for name in ["log", "localised_print", "table_size"] {
        assert!(
            globals.contains(&format!("pub fn {name}"))
                || globals.contains(&format!("pub fn {name} ")),
            "missing global function `{name}`"
        );
    }
}

#[test]
fn maps_events_to_rust_names() {
    let api = parse_runtime_api(factorio_api_gen::bundled_runtime_api_json())
        .expect("bundled runtime-api.json should parse");
    let generated = generate_runtime_api(&api);

    assert!(generated.events.contains("OnSingleplayerInit"));
    assert!(generated.event_map.contains("on_singleplayer_init"));
    assert!(generated.event_lookup.contains("OnSingleplayerInit"));
}

#[test]
fn nests_known_concepts_in_copy_fields() {
    let generated = generate_from_bundled_api().expect("generate");
    let concepts = &generated.concepts;

    assert!(
        concepts.contains("pub color : crate :: concepts :: Color")
            || concepts.contains("pub color: crate::concepts::Color"),
        "PrintSettings.color should be Color, got concepts without nested Color"
    );
    assert!(
        concepts.contains("pub left_top : crate :: concepts :: MapPosition")
            || concepts.contains("pub left_top: crate::concepts::MapPosition"),
        "BoundingBox.left_top should be MapPosition"
    );
    assert!(
        !concepts.contains("pub left_top : crate :: LuaAny")
            && !concepts.contains("pub left_top: crate::LuaAny"),
        "BoundingBox.left_top must not be LuaAny"
    );
}

#[test]
fn map_location_self_cycle_stays_lua_any() {
    let generated = generate_from_bundled_api().expect("generate");
    assert!(
        generated.concepts.contains("pub struct MapLocation"),
        "MapLocation should be generated"
    );
    assert!(
        generated
            .concepts
            .contains("pub position : crate :: LuaAny")
            || generated.concepts.contains("pub position: crate::LuaAny"),
        "MapLocation.position should stay LuaAny to preserve Copy"
    );
}

#[test]
fn emits_numeric_concept_aliases() {
    let generated = generate_from_bundled_api().expect("generate");
    assert!(
        generated
            .concepts
            .contains("pub type RealOrientation = f32")
            || generated.concepts.contains("pub type RealOrientation=f32"),
        "RealOrientation should be a numeric alias"
    );
    assert!(
        generated.concepts.contains("pub type Weight = f64")
            || generated.concepts.contains("pub type Weight=f64"),
        "Weight should be a numeric alias"
    );
}

#[test]
fn emits_identification_enums() {
    let generated = generate_from_bundled_api().expect("generate");
    let concepts = &generated.concepts;

    assert!(
        concepts.contains("pub enum ForceID"),
        "ForceID enum missing"
    );
    assert!(
        concepts.contains("pub enum PlayerIdentification"),
        "PlayerIdentification enum missing"
    );
    assert!(
        concepts.contains("pub enum ScriptRenderTarget"),
        "ScriptRenderTarget enum missing"
    );
    assert!(
        concepts.contains("Force (crate :: classes :: LuaForce)")
            || concepts.contains("Force(crate::classes::LuaForce)"),
        "ForceID should have LuaForce arm"
    );
    assert!(
        concepts.contains("Position (crate :: concepts :: MapPosition)")
            || concepts.contains("Position(crate::concepts::MapPosition)"),
        "ScriptRenderTarget should have MapPosition arm"
    );
}

#[test]
fn function_parameters_use_lua_function() {
    let generated = generate_from_bundled_api().expect("generate");
    let classes = &generated.classes;

    assert!(
        classes.contains("impl Into < crate :: LuaFunction >")
            || classes.contains("impl Into<crate::LuaFunction>"),
        "add_command-style handlers should accept Into<LuaFunction>"
    );
    // quote! may insert spaces around `::` / `<`.
    assert!(
        classes.contains("impl crate :: IntoOptionalLuaFunction")
            || classes.contains("impl crate::IntoOptionalLuaFunction"),
        "on_event-style handlers should accept IntoOptionalLuaFunction"
    );
    assert!(
        !classes.contains("handler : crate :: LuaAny")
            && !classes.contains("handler: crate::LuaAny"),
        "event handlers must not be typed as LuaAny"
    );
}
