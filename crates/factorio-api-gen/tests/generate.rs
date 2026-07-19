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
    for name in ["storage", "serpent", "math", "string", "table"] {
        assert!(
            globals.contains(&format!("pub static {name}"))
                || globals.contains(&format!("pub static {name} ")),
            "missing auxiliary global `{name}`"
        );
    }

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
fn emits_attribute_setters_without_write_only_getters() {
    let generated = generate_from_bundled_api().expect("generate");
    let classes = &generated.classes;
    let style_start = classes
        .find("impl LuaStyle")
        .expect("LuaStyle impl missing");
    let style_chunk = &classes[style_start..style_start.saturating_add(50_000)];
    let style = style_chunk.replace(' ', "");

    assert!(
        classes
            .replace(' ', "")
            .contains("pubfnset_caption(&self,value:implInto<crate::LocalisedString>)"),
        "LuaGuiElement.set_caption missing"
    );
    assert!(
        classes
            .replace(' ', "")
            .contains("pubfncaption(&self)->crate::LocalisedString"),
        "LuaGuiElement.caption getter missing"
    );
    assert!(
        style.contains("pubfnset_width(&self,value:i32)"),
        "LuaStyle.set_width missing"
    );
    assert!(
        !style.contains("pubfnwidth(&self)"),
        "write-only LuaStyle.width must not have a getter"
    );
    assert!(
        classes
            .replace(' ', "")
            .contains("pubfnwrite_driving(&self"),
        "LuaControl.driving setter should be write_driving (set_driving is a real method)"
    );
    let classes_compact = classes.replace(' ', "");
    assert!(
        classes_compact.contains("pubfnstyle(&self)->crate::classes::LuaStyle"),
        "LuaGuiElement.style getter should return LuaStyle"
    );
    assert!(
        classes_compact.contains("pubfnset_style(&self,value:&'staticstr)"),
        "LuaGuiElement.set_style should take a style name string"
    );
    let lookup = generated.attribute_setters.replace(' ', "");
    assert!(
        lookup.contains("\"set_caption\"=>Some(\"caption\")"),
        "attribute setter lookup should map set_caption"
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
fn optional_concept_fields_are_option() {
    let generated = generate_from_bundled_api().expect("generate");
    let concepts = &generated.concepts;
    assert!(
        concepts.contains("pub a : Option <")
            || concepts.contains("pub a: Option<")
            || concepts.contains("pub a : Option<"),
        "Color.a should be Option<_>, got concepts without Option a"
    );
    assert!(
        concepts.contains("pub color : Option <")
            || concepts.contains("pub color: Option<")
            || concepts.contains("pub color : Option<"),
        "PrintSettings.color should be Option<_>"
    );
}

#[test]
fn optional_takes_table_fields_are_option() {
    let generated = generate_from_bundled_api().expect("generate");
    let classes = &generated.classes;
    assert!(
        classes.contains("pub force : Option <")
            || classes.contains("pub force: Option<")
            || classes.contains("pub force : Option<"),
        "LuaEntityMineParams.force should be Option<_>"
    );
}

#[test]
fn emits_is_identification_type_helper() {
    let generated = generate_from_bundled_api().expect("generate");
    assert!(
        generated.debug_types.contains("fn is_identification_type"),
        "debug_types should expose is_identification_type"
    );
    assert!(
        generated.debug_types.contains("ForceID"),
        "is_identification_type should match ForceID"
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
    assert!(
        classes.contains("impl Into < crate :: LocalisedString >")
            || classes.contains("impl Into<crate::LocalisedString>"),
        "LocalisedString parameters should accept Into<LocalisedString>"
    );
    // Sorted by Factorio `order`: name, help, function (not JSON array order).
    let add_command = classes
        .find("fn add_command")
        .map(|i| &classes[i..i.saturating_add(280)])
        .expect("add_command method");
    let name_at = add_command.find("name").expect("name param");
    let help_at = add_command.find("help").expect("help param");
    let function_at = add_command.find("function").expect("function param");
    assert!(
        name_at < help_at && help_at < function_at,
        "add_command parameters should follow Factorio order (name, help, function), got:\n{add_command}"
    );
}
