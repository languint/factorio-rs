#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use factorio_frontend::{ParseOptions, parse_module_with_options};
use factorio_ir::{
    expression::Expression, lint::LintConfig, literal::Literal, statement::Statement,
};

#[test]
fn recipe_macro_emits_recipes_const_and_register_recipes() {
    let source = r#"
        recipe! {
            craft_widget {
                name = "my-mod-widget",
                energy_required = 1.0,
                ingredients = [
                    { name = "iron-plate", amount = 2 },
                    { name = "copper-plate", amount = 1 },
                ],
                results = [
                    { name = "my-mod-widget", amount = 1 },
                ],
                category = "crafting",
                enabled = true,
            }
        }
    "#;

    let lints = LintConfig::allow_all();
    let mut diagnostics = Vec::new();
    let module =
        parse_module_with_options(source, "data", &ParseOptions::new(&lints), &mut diagnostics)
            .expect("parse");

    let recipes = module
        .symbols
        .iter()
        .find_map(|symbol| match &symbol.statement {
            Statement::StructDecl(s) if s.name == "Recipes" => Some(s),
            _ => None,
        })
        .expect("Recipes struct");
    assert!(
        recipes.constants.iter().any(|(name, value)| {
            name == "CRAFT_WIDGET"
                && matches!(
                    value,
                    Expression::Literal(Literal::String(s)) if s == "my-mod-widget"
                )
        }),
        "expected Recipes::CRAFT_WIDGET const, got {:?}",
        recipes.constants
    );

    let register = module
        .symbols
        .iter()
        .find_map(|symbol| match &symbol.statement {
            Statement::FunctionDecl(f) if f.name == "register_recipes" => Some(f),
            _ => None,
        })
        .expect("register_recipes");
    let body = format!("{:?}", register.body);
    assert!(
        body.contains("iron-plate") && body.contains("RecipeIngredient"),
        "expected ingredients in register_recipes body: {body}"
    );
    assert!(
        body.contains("my-mod-widget") && body.contains("RecipeProduct"),
        "expected results in register_recipes body: {body}"
    );
}
