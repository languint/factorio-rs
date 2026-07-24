use std::path::{Path, PathBuf};

use factorio_frontend::{BenchSuite, ParseOptions, discover_benches};
use factorio_ir::lint::LintConfig;

#[allow(clippy::expect_used)]
fn run(source: &str) -> BenchSuite {
    let lints = LintConfig::allow_all();
    let options = ParseOptions::new(&lints);
    let mut diagnostics = Vec::new();
    let sources = vec![(PathBuf::from("src/lib.rs"), source.to_string())];
    let suite = discover_benches(Path::new("src"), &sources, &options, &mut diagnostics)
        .expect("discover_benches failed");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
    suite
}

#[test]
fn discovers_bench_iterations_stored() {
    let suite = run(r"
        #[factorio_rs::control]
        mod control {
            #[factorio_rs::bench(iterations = 3)]
            pub fn heavy_bench() {}
        }
    ");
    assert_eq!(suite.benches.len(), 1);
    let b = &suite.benches[0];
    assert_eq!(b.name, "control::heavy_bench");
    assert_eq!(b.lua_name, "heavy_bench");
    assert_eq!(b.iterations, 3);
}

#[test]
fn to_module_contains_bench_function() {
    let suite = run(r"
        #[factorio_rs::bench(iterations = 3)]
        pub fn my_bench() {}
    ");
    assert_eq!(suite.benches.len(), 1);
    let module = suite.to_module();
    assert_eq!(module.name, "factorio_rs_benches");
    assert_eq!(module.symbols.len(), 1);
    let sym = &module.symbols[0];
    assert!(
        matches!(
            &sym.statement,
            factorio_ir::statement::Statement::FunctionDecl(f) if f.name == "my_bench"
        ),
        "expected my_bench in symbols: {:?}",
        module.symbols
    );
}

#[test]
fn default_iterations_is_one() {
    let suite = run(r"
        #[factorio_rs::bench]
        pub fn quick_bench() {}
    ");
    assert_eq!(suite.benches.len(), 1);
    assert_eq!(suite.benches[0].iterations, 1);
}

#[test]
fn bench_fn_skipped_in_normal_control_emit() {
    use factorio_frontend::parse_module;
    let source = r"
        #[factorio_rs::bench]
        pub fn my_bench() {}
    ";
    // parse_module uses Stage::Control (any control-stage module name works).
    let module = parse_module(source, "control").expect("parse_module failed");
    // The bench fn must NOT appear in normal module symbols or body.
    assert!(
        !module.symbols.iter().any(|s| matches!(
            &s.statement,
            factorio_ir::statement::Statement::FunctionDecl(f) if f.name == "my_bench"
        )),
        "bench fn must not appear in normal control module symbols"
    );
    assert!(
        !module.body.statements.iter().any(|s| matches!(
            s,
            factorio_ir::statement::Statement::FunctionDecl(f) if f.name == "my_bench"
        )),
        "bench fn must not appear in normal control module body"
    );
}

#[test]
fn bench_in_cfg_test_module_discovered() {
    let suite = run(r"
        #[cfg(test)]
        mod perf_tests {
            #[factorio_rs::bench(iterations = 10)]
            pub fn loop_bench() {}
        }
    ");
    assert_eq!(suite.benches.len(), 1);
    assert_eq!(suite.benches[0].name, "perf_tests::loop_bench");
    assert_eq!(suite.benches[0].iterations, 10);
}

#[test]
fn multiple_benches_collected_with_unique_lua_names() {
    let suite = run(r"
        #[factorio_rs::bench]
        pub fn bench_a() {}

        mod sub {
            #[factorio_rs::bench]
            pub fn bench_a() {}
        }
    ");
    assert_eq!(suite.benches.len(), 2);
    // lua_names must be unique.
    let lua_names: Vec<&str> = suite.benches.iter().map(|b| b.lua_name.as_str()).collect();
    assert_ne!(
        lua_names[0], lua_names[1],
        "lua_names must be unique: {lua_names:?}"
    );
}

#[test]
fn suite_is_empty_when_no_benches_present() {
    let suite = run(r"
        pub fn helper() {}
        #[test]
        pub fn a_test() {}
    ");
    assert!(suite.is_empty());
}
