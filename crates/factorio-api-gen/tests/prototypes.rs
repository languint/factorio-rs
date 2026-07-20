#![allow(clippy::expect_used, clippy::unwrap_used)]

#[test]
fn bundled_prototype_api_generates_all_typenames() {
    let source = factorio_api_gen::generate_prototypes_from_bundled().expect("generate");
    let struct_count = source.matches("pub struct ").count();
    assert!(
        (250..=280).contains(&struct_count),
        "expected ~260 prototype stubs, got {struct_count}"
    );
    for name in [
        "pub struct Item",
        "pub struct Recipe",
        "pub struct Technology",
        "pub struct Fluid",
        "pub struct AssemblingMachine",
        "pub struct Container",
        "pub struct Inserter",
        "pub struct TransportBelt",
        "pub struct Tile",
    ] {
        assert!(
            source.contains(name),
            "missing `{name}` in generated prototypes"
        );
    }
    assert!(
        source.contains("prototype API"),
        "expected version header comment"
    );
    assert!(
        factorio_api_gen::PROTOTYPE_RICH_OVERRIDES.contains(&"item"),
        "rich overrides should list item"
    );
}

#[test]
fn bundled_prototype_type_map_covers_core_and_companions() {
    let source = factorio_api_gen::generate_prototype_type_map_from_bundled().expect("type map");
    assert!(source.contains("pub fn prototype_lua_typename"));
    for needle in [
        "\"Item\" => Some(\"item\")",
        "\"Container\" => Some(\"container\")",
        "\"Inserter\" => Some(\"inserter\")",
        "\"RecipeProduct\" => Some(\"item\")",
        "\"BoolSetting\" => Some(\"bool-setting\")",
        "\"UnlockRecipeEffect\" => Some(\"unlock-recipe\")",
    ] {
        // quote may insert spaces around `=>`
        let compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
        let want: String = needle.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains(&want), "type map missing `{needle}`");
    }
}
