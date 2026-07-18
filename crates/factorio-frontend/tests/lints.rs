#![allow(
    clippy::expect_used,
    clippy::literal_string_with_formatting_args,
    clippy::needless_raw_string_hashes,
    clippy::panic,
    clippy::unwrap_used
)]
use factorio_frontend::ParseOptions;
use factorio_frontend::parse_module_with_options;
use factorio_ir::{
    lint::{LintConfig, LintId, LintLevel},
    statement::Statement,
};

#[test]
fn deny_unwrap_collects_diagnostic() {
    let source = r"
pub fn f(x: Option<i32>) -> i32 {
    x.unwrap()
}
";
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .expect("deny lints should not abort lowering");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, LintId::Unwrap);
    assert_eq!(diagnostics[0].level, LintLevel::Deny);
}

#[test]
fn collects_multiple_deny_lints() {
    let source = r"
pub fn f(x: Option<i32>, y: Option<i32>) -> i32 {
    let a = x.unwrap();
    let b = y.unwrap();
    a + b
}
";
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .expect("should collect both unwraps");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|d| d.id == LintId::Unwrap));
    assert!(diagnostics.iter().all(|d| d.level == LintLevel::Deny));
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
fn warn_format_spec() {
    let source = r#"
pub fn f(n: f64) {
    println!("{:.2}", n);
}
"#;
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .expect("format_spec is warn by default and should not fail the build");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, LintId::FormatSpec);
    assert_eq!(diagnostics[0].level, LintLevel::Warn);
    let snippet = &source[diagnostics[0].loc.span.range()];
    assert_eq!(
        snippet, "{:.2}",
        "lint should cover the placeholder, got {snippet:?}"
    );
}

#[test]
fn deny_variable_index_collects_diagnostic() {
    let source = r"
pub fn f(items: LuaAny, i: usize) -> LuaAny {
    items[i]
}
";
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .expect("deny lints should not abort lowering");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, LintId::VariableIndex);
}

#[test]
fn string_literal_index_does_not_lint_variable_index() {
    let source = r#"
pub fn f(items: LuaAny) -> LuaAny {
    items["counter"]
}
"#;
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .expect("string keys should lower");
    assert!(
        diagnostics.iter().all(|d| d.id != LintId::VariableIndex),
        "string literal indexes should not emit variable_index, got {diagnostics:?}"
    );
}

#[test]
fn deny_identification_ctor_collects_diagnostic() {
    let source = r#"
pub fn f() {
    let _id = ForceID::Name("enemy");
}
"#;
    let lints = LintConfig::default();
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&lints),
        &mut diagnostics,
    )
    .expect("deny lints should not abort lowering");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, LintId::IdentificationCtor);
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
        panic!("expected return, got {:?}", function.body.statements);
    };
    assert!(matches!(
        expr,
        factorio_ir::expression::Expression::If { .. }
    ));
}

#[test]
fn deny_option_if() {
    let source = r"
pub fn f(x: Option<i32>) {
    if x {
        let _ = 1;
    }
}
";
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&LintConfig::default()),
        &mut diagnostics,
    )
    .expect("should lower");
    assert!(diagnostics.iter().any(|d| d.id == LintId::OptionIf));
}

#[test]
fn deny_ambiguous_try_on_untyped_local() {
    let source = r#"
pub fn f() -> Result<i32, String> {
    let x = 1;
    x?
}
"#;
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&LintConfig::default()),
        &mut diagnostics,
    )
    .expect("should lower");
    assert!(diagnostics.iter().any(|d| d.id == LintId::AmbiguousTry));
}

#[test]
fn option_try_lowers_nil_check() {
    let source = r"
pub fn f(x: Option<i32>) -> Option<i32> {
    let y = x?;
    Some(y)
}
";
    let mut diagnostics = Vec::new();
    let module = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&LintConfig::default()),
        &mut diagnostics,
    )
    .expect("should lower");
    assert!(
        !diagnostics.iter().any(|d| d.id == LintId::AmbiguousTry),
        "typed Option should not lint ambiguous_try"
    );
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    assert!(
        function.body.statements.iter().any(|stmt| {
            matches!(
                stmt,
                Statement::Conditional {
                    condition: factorio_ir::expression::Expression::BinaryOp {
                        op: factorio_ir::operator::Operator::Eq,
                        ..
                    },
                    ..
                }
            )
        }),
        "expected nil equality early-return, got {:?}",
        function.body.statements
    );
}

#[test]
fn deny_ambiguous_method_on_untyped_local() {
    let source = r"
pub fn f() -> Option<i32> {
    let x = Some(1);
    x.map(|n| n + 1)
}
";
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&LintConfig::default()),
        &mut diagnostics,
    )
    .expect("should lower");
    assert!(diagnostics.iter().any(|d| d.id == LintId::AmbiguousMethod));
}

#[test]
fn deny_skipped_mod() {
    let source = r"
mod inner {
    pub fn ignored() {}
}
";
    let mut diagnostics = Vec::new();
    parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&LintConfig::default()),
        &mut diagnostics,
    )
    .expect("should lower");
    assert!(diagnostics.iter().any(|d| d.id == LintId::SkippedMod));
}

#[test]
fn identification_ctor_does_not_emit_call() {
    let source = r#"
pub fn f() {
    let _id = ForceID::Name("enemy");
}
"#;
    let mut diagnostics = Vec::new();
    let module = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&LintConfig::default()),
        &mut diagnostics,
    )
    .expect("should lower");
    assert_eq!(diagnostics[0].id, LintId::IdentificationCtor);
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::VariableDecl { value, .. } = &function.body.statements[0] else {
        panic!("expected let");
    };
    assert!(matches!(
        value,
        factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::Nil)
    ));
}

#[test]
fn closure_try_hoists_stay_inside_closure() {
    let source = r#"
pub fn outer(r: Result<i32, String>) {
    let f = |x: Result<i32, String>| x?;
    let _ = f(r);
}
"#;
    let mut diagnostics = Vec::new();
    let module = parse_module_with_options(
        source,
        "control",
        &ParseOptions::new(&LintConfig::allow_all()),
        &mut diagnostics,
    )
    .expect("should lower");
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    // Outer body must not contain try early-return conditionals from the closure.
    let outer_has_try = function
        .body
        .statements
        .iter()
        .any(|stmt| matches!(stmt, Statement::Conditional { .. }));
    assert!(
        !outer_has_try,
        "try hoists leaked into outer fn: {:?}",
        function.body.statements
    );
    let Statement::VariableDecl { value, .. } = &function.body.statements[0] else {
        panic!("expected closure binding");
    };
    let factorio_ir::expression::Expression::Closure { body, .. } = value else {
        panic!("expected closure");
    };
    assert!(
        body.statements
            .iter()
            .any(|stmt| matches!(stmt, Statement::Conditional { .. })),
        "closure should contain try early-return"
    );
}
