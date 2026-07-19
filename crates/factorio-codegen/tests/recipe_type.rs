use factorio_codegen::LuaGenerator;
use factorio_ir::{expression::Expression, literal::Literal};

#[test]
fn injects_recipe_and_ingredient_type_discriminants() {
    let recipe = Expression::StructLiteral {
        struct_name: Some("Recipe".to_string()),
        fields: vec![
            (
                "name".to_string(),
                Expression::Literal(Literal::String("my-mod-widget".to_string())),
            ),
            (
                "ingredients".to_string(),
                Expression::Array {
                    elements: vec![Expression::StructLiteral {
                        struct_name: Some("RecipeIngredient".to_string()),
                        fields: vec![
                            (
                                "name".to_string(),
                                Expression::Literal(Literal::String("iron-plate".to_string())),
                            ),
                            ("amount".to_string(), Expression::Literal(Literal::Int(2))),
                        ],
                    }],
                },
            ),
            (
                "results".to_string(),
                Expression::Array {
                    elements: vec![Expression::StructLiteral {
                        struct_name: Some("RecipeProduct".to_string()),
                        fields: vec![
                            (
                                "name".to_string(),
                                Expression::Literal(Literal::String("my-mod-widget".to_string())),
                            ),
                            ("amount".to_string(), Expression::Literal(Literal::Int(1))),
                        ],
                    }],
                },
            ),
        ],
    };

    let lua = LuaGenerator::new().generate_expression(&recipe);
    assert!(
        lua.contains("type = \"recipe\""),
        "expected recipe type injection: {lua}"
    );
    assert!(
        lua.contains("type = \"item\""),
        "expected ingredient/product type injection: {lua}"
    );
    assert!(
        lua.contains("name = \"iron-plate\"") && lua.contains("amount = 2"),
        "expected ingredient fields: {lua}"
    );
}
