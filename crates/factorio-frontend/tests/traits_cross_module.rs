#![allow(
    clippy::expect_used,
    clippy::literal_string_with_formatting_args,
    clippy::needless_raw_string_hashes,
    clippy::panic,
    clippy::unwrap_used
)]
mod common;

use std::path::{Path, PathBuf};

use common::must_ok_parse;
use factorio_frontend::{
    ParseOptions, TraitCatalog, build_trait_catalog, parse_module_with_options,
};
use factorio_ir::{expression::Expression, lint::LintConfig, statement::Statement};

fn catalog_from_shared_alert(source: &str) -> TraitCatalog {
    let sources = vec![(
        PathBuf::from("/project/src/shared/alert.rs"),
        source.to_string(),
    )];
    build_trait_catalog(&sources, Path::new("/project/src")).expect("build catalog")
}

#[test]
fn cross_module_trait_via_use() {
    let shared = r#"
pub trait Alert {
    fn prio(&self) -> i64;
}
"#;
    let catalog = catalog_from_shared_alert(shared);

    let control = r#"
use crate::shared::alert::Alert;

struct PowerDrop {
    percent: i64,
}

impl Alert for PowerDrop {
    fn prio(&self) -> i64 {
        100 - self.percent
    }
}

pub fn run(p: PowerDrop) -> i64 {
    p.prio()
}
"#;
    let lints = LintConfig::allow_all();
    let mut diagnostics = Vec::new();
    let module = must_ok_parse(parse_module_with_options(
        control,
        "control",
        &ParseOptions::new(&lints).with_trait_catalog(&catalog),
        &mut diagnostics,
    ));
    assert!(diagnostics.is_empty());

    let Statement::StructDecl(struct_decl) = &module.body.statements[0] else {
        panic!("expected struct, got {:?}", module.body.statements[0]);
    };
    assert_eq!(struct_decl.methods[0].name, "prio");
    assert_eq!(module.vtables[0].name, "__vt_Alert_PowerDrop");

    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::Return(Some(Expression::Call { func, .. })) = &function.body.statements[0]
    else {
        panic!("expected method call");
    };
    assert_eq!(
        func.as_ref(),
        &Expression::QualifiedPath {
            segments: vec!["PowerDrop".to_string(), "prio".to_string()],
        }
    );
}

#[test]
fn cross_module_unknown_without_catalog() {
    let control = r#"
use crate::shared::alert::Alert;

struct PowerDrop {
    percent: i64,
}

impl Alert for PowerDrop {
    fn prio(&self) -> i64 {
        self.percent
    }
}
"#;
    let err = factorio_frontend::parse_module(control, "control").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown trait") || msg.contains("Alert"),
        "unexpected error: {msg}"
    );
}

#[test]
fn build_trait_catalog_from_sources() {
    let shared = r#"
pub trait Alert {
    fn prio(&self) -> i64;
}
"#;
    let catalog = catalog_from_shared_alert(shared);
    assert!(catalog.get("shared.alert", "Alert").is_some());
}
