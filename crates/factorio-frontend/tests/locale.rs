#![allow(clippy::unwrap_used)]

use factorio_frontend::parse_module;
use factorio_ir::locale::{LocaleEntry, LocaleFile};

#[test]
fn locale_macro_resolves_setting_const_keys() {
    let source = r#"
        use factorio_rs::prelude::*;

        factorio_rs::mod_settings! {
            prefix = "msr",
            startup {
                casual_mode: bool = false,
            }
        }

        factorio_rs::locale! {
            file = "settings",
            en {
                mod_setting_name {
                    Settings::CASUAL_MODE = "Casual mode",
                }
                "my-mod-messages" {
                    "hello" = "Hello!",
                }
            }
        }
    "#;

    let module = parse_module(source, "settings").unwrap();
    assert_eq!(
        module.locales,
        vec![LocaleFile {
            lang: "en".to_string(),
            file: "settings".to_string(),
            entries: vec![
                LocaleEntry {
                    category: Some("mod-setting-name".to_string()),
                    key: "msr-casual-mode".to_string(),
                    value: "Casual mode".to_string(),
                },
                LocaleEntry {
                    category: Some("my-mod-messages".to_string()),
                    key: "hello".to_string(),
                    value: "Hello!".to_string(),
                },
            ],
        }]
    );
}

#[test]
fn locale_file_serializes_factorio_cfg() {
    let file = LocaleFile {
        lang: "en".to_string(),
        file: "settings".to_string(),
        entries: vec![
            LocaleEntry {
                category: Some("mod-setting-name".to_string()),
                key: "msr-casual-mode".to_string(),
                value: "Casual mode".to_string(),
            },
            LocaleEntry {
                category: Some("mod-setting-description".to_string()),
                key: "msr-casual-mode".to_string(),
                value: "Relax rules.".to_string(),
            },
        ],
    };

    assert_eq!(
        file.to_cfg(),
        "[mod-setting-name]\n\
         msr-casual-mode=Casual mode\n\
         \n\
         [mod-setting-description]\n\
         msr-casual-mode=Relax rules.\n"
    );
}

#[test]
fn locale_macro_multiple_languages() {
    let source = r#"
        factorio_rs::mod_settings! {
            prefix = "msr",
            startup {
                casual_mode: bool = false,
            }
        }

        factorio_rs::locale! {
            file = "settings",
            en {
                mod_setting_name {
                    Settings::CASUAL_MODE = "Casual mode",
                }
            }
            de {
                mod_setting_name {
                    Settings::CASUAL_MODE = "Entspannter Modus",
                }
            }
        }
    "#;

    let module = parse_module(source, "settings").unwrap();
    assert_eq!(module.locales.len(), 2);
    assert_eq!(module.locales[0].lang, "en");
    assert_eq!(module.locales[1].lang, "de");
    assert_eq!(module.locales[1].entries[0].value, "Entspannter Modus");
}
