#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use factorio_frontend::{
    FrontendError, parse_locale_pending, parse_module, resolve_project_locales,
};
use factorio_ir::{
    expression::Expression,
    literal::Literal,
    locale::{LocaleEntry, LocaleFile, PendingLocaleFile},
    module::{ImportedItem, Module, ModuleImport, Symbol},
    scope::Scope,
    stage::Stage,
    statement::Statement,
    structure::Struct,
};

#[test]
fn locale_resolves_imported_items_const_across_modules() {
    let items_module = parse_module(
        r#"
        item! {
            widget {
                name = "my-mod-widget",
                icon = "__my_mod__/graphics/icon.png",
                stack_size = 50,
            }
        }
        "#,
        "data.items",
    )
    .expect("items module");

    let locale_module = Module {
        name: "data.items_locale".to_string(),
        stage: Stage::Data,
        body: factorio_ir::block::Block { statements: vec![] },
        symbols: vec![],
        imports: vec![ModuleImport {
            module: "data.items".to_string(),
            local: "data_items".to_string(),
            items: vec![ImportedItem {
                name: "Items".to_string(),
                local: "Items".to_string(),
            }],
            factorio_mod: None,
            module_root: None,
        }],
        submodules: vec![],
        locales: vec![],
        pending_locales: pending_from(
            r#"
            file = "items",
            en {
                item_name {
                    Items::WIDGET = "Widget",
                }
                item_description {
                    Items::WIDGET = "A sample item.",
                }
            }
            "#,
        ),
    };

    let mut modules = vec![items_module, locale_module];
    resolve_project_locales(&mut modules).expect("resolve");

    assert_eq!(
        modules[1].locales,
        vec![LocaleFile {
            lang: "en".to_string(),
            file: "items".to_string(),
            entries: vec![
                LocaleEntry {
                    category: Some("item-name".to_string()),
                    key: "my-mod-widget".to_string(),
                    value: "Widget".to_string(),
                },
                LocaleEntry {
                    category: Some("item-description".to_string()),
                    key: "my-mod-widget".to_string(),
                    value: "A sample item.".to_string(),
                },
            ],
        }]
    );
    assert!(modules[1].pending_locales.is_empty());
}

#[test]
fn locale_unresolved_without_import_or_local_const() {
    let mut locale_module = Module {
        name: "data.items_locale".to_string(),
        stage: Stage::Data,
        body: factorio_ir::block::Block { statements: vec![] },
        symbols: vec![],
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: pending_from(
            r#"
            file = "items",
            en {
                item_name {
                    Items::WIDGET = "Widget",
                }
            }
            "#,
        ),
    };

    let err = resolve_project_locales(std::slice::from_mut(&mut locale_module)).unwrap_err();
    assert!(
        matches!(err, FrontendError::LocaleKeyUnresolved { .. }),
        "unexpected: {err:?}"
    );
}

#[test]
fn locale_resolves_renamed_import() {
    let items_module = Module {
        name: "data.items".to_string(),
        stage: Stage::Data,
        body: factorio_ir::block::Block { statements: vec![] },
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::StructDecl(Struct {
                name: "Items".to_string(),
                fields: vec![],
                methods: vec![],
                constants: vec![(
                    "WIDGET".to_string(),
                    Expression::Literal(Literal::String("my-mod-widget".to_string())),
                )],
                doc: None,
                debug: None,
            }),
        }],
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
    };

    let locale_module = Module {
        name: "data.items_locale".to_string(),
        stage: Stage::Data,
        body: factorio_ir::block::Block { statements: vec![] },
        symbols: vec![],
        imports: vec![ModuleImport {
            module: "data.items".to_string(),
            local: "data_items".to_string(),
            items: vec![ImportedItem {
                name: "Items".to_string(),
                local: "I".to_string(),
            }],
            factorio_mod: None,
            module_root: None,
        }],
        submodules: vec![],
        locales: vec![],
        pending_locales: pending_from(
            r#"
            file = "items",
            en {
                item_name {
                    I::WIDGET = "Widget",
                }
            }
            "#,
        ),
    };

    let mut modules = vec![items_module, locale_module];
    resolve_project_locales(&mut modules).expect("resolve");
    assert_eq!(modules[1].locales[0].entries[0].key, "my-mod-widget");
}

fn pending_from(inner: &str) -> Vec<PendingLocaleFile> {
    let tokens: proc_macro2::TokenStream = inner.parse().expect("tokens");
    parse_locale_pending(tokens).expect("parse locale pending")
}
