use factorio_frontend::{
    FrontendError, ParseOptions, parse_module_with_options,
};
use factorio_ir::{
    lint::{LintConfig, LintId, LintLevel},
    statement::Statement,
};

#[test]
fn deny_unwrap_fails_parse() {
    let source = r"
pub fn f(x: Option<i32>) -> i32 {
    x.unwrap()
}
";
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    let err = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .unwrap_err();
    match err {
        FrontendError::Lint(diag) => {
            assert_eq!(diag.id, LintId::Unwrap);
            assert_eq!(diag.level, LintLevel::Deny);
        }
        other => panic!("expected Lint error, got {other:?}"),
    }
}

#[test]
fn allow_unwrap_succeeds() {
    let source = r"
pub fn f(x: Option<i32>) -> i32 {
    x.unwrap()
}
";
    let lints = LintConfig::default().allowing(LintId::Unwrap);
    let mut diagnostics = Vec::new();
    let module = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .expect("parse with unwrap allowed");
    assert!(!module.symbols.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn warn_expect_collects_diagnostic() {
    let source = r#"
pub fn f(x: Option<i32>) -> i32 {
    x.expect("missing")
}
"#;
    let mut levels = std::collections::BTreeMap::new();
    levels.insert("expect".to_string(), LintLevel::Warn);
    let lints = LintConfig::default()
        .with_overrides(&levels)
        .expect("overrides");
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .expect("warn should not fail");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, LintId::Expect);
    assert_eq!(diagnostics[0].level, LintLevel::Warn);
}

#[test]
fn deny_format_spec() {
    let source = r#"
pub fn f(n: f64) {
    println!("{:.2}", n);
}
"#;
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    let err = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .unwrap_err();
    match err {
        FrontendError::Lint(diag) => assert_eq!(diag.id, LintId::FormatSpec),
        other => panic!("expected format_spec lint, got {other:?}"),
    }
}

#[test]
fn deny_variable_index() {
    let source = r"
pub fn f(items: LuaAny, i: usize) -> LuaAny {
    items[i]
}
";
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    let err = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .unwrap_err();
    match err {
        FrontendError::Lint(diag) => assert_eq!(diag.id, LintId::VariableIndex),
        other => panic!("expected variable_index lint, got {other:?}"),
    }
}

#[test]
fn deny_identification_ctor() {
    let source = r#"
pub fn f() {
    let _id = ForceID::Name("enemy");
}
"#;
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    let err = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .unwrap_err();
    match err {
        FrontendError::Lint(diag) => assert_eq!(diag.id, LintId::IdentificationCtor),
        other => panic!("expected identification_ctor lint, got {other:?}"),
    }
}

#[test]
fn lowers_safe_if_expression() {
    let source = r"
pub fn pick(flag: bool) -> i32 {
    return if flag { 0 } else { 1 };
}
";
    let module = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&LintConfig::allow_all()),
        &mut Vec::new(),
    )
    .expect("parse");
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::Return(Some(expr)) = &function.body.statements[0] else {
        panic!(
            "expected return, got {:?}",
            function.body.statements
        );
    };
    assert!(matches!(
        expr,
        factorio_ir::expression::Expression::If { .. }
    ));
}

