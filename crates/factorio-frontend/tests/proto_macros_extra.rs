#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use factorio_frontend::{ParseOptions, parse_module_with_options};
use factorio_ir::{lint::LintConfig, statement::Statement};

fn parse_ok(source: &str) {
    let lints = LintConfig::allow_all();
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "data",
        &ParseOptions::new(&lints).with_mod_name("my_mod"),
        &mut diagnostics,
    )
    .expect("parse");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn container_macro_expands() {
    parse_ok(
        r#"
        container! {
            chest {
                name = "my-mod-chest",
                inventory_size = 16,
                icon = "graphics/chest.png",
                flags = ["placeable-neutral"],
            }
        }
    "#,
    );
}

#[test]
fn transport_belt_macro_expands() {
    parse_ok(
        r#"
        transport_belt! {
            belt {
                name = "my-mod-belt",
                speed = 0.03125,
                icon = "graphics/belt.png",
            }
        }
    "#,
    );
}

#[test]
fn recipe_category_macro_expands() {
    let lints = LintConfig::allow_all();
    let mut diagnostics = Vec::new();
    let module = parse_module_with_options(
        r#"
        recipe_category! {
            smelting_extra {
                name = "my-mod-smelting",
                order = "a",
            }
        }
    "#,
        "data",
        &ParseOptions::new(&lints).with_mod_name("my_mod"),
        &mut diagnostics,
    )
    .expect("parse");

    assert!(
        module.symbols.iter().any(|symbol| {
            matches!(
                &symbol.statement,
                Statement::StructDecl(s) if s.name == "RecipeCategories"
            )
        }),
        "expected RecipeCategories"
    );
    assert!(
        module.symbols.iter().any(|symbol| {
            matches!(
                &symbol.statement,
                Statement::FunctionDecl(f) if f.name == "register_recipe_categories"
            )
        }),
        "expected register_recipe_categories"
    );
}
