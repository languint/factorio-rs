use std::fmt::Write;
use std::io::{BufRead, BufReader, IsTerminal, Read, stdout};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use factorio_codegen::LuaGenerator;
use factorio_frontend::{
    ParseOptions, discover_tests, display_filename, eprint_diagnostic, eprint_frontend_error,
};

use crate::{
    cargo_manifest::CargoPackage,
    commands::build::{BuildOptions, build},
    commands::typecheck,
    config::Config,
    error::{CliError, CliResult},
    paths::{FactorioLaunchTarget, find_factorio},
};

const PROTOCOL_PREFIX: &str = "FACTORIO_RS_TEST";

/// Options for [`run_tests`].
#[derive(Debug, Clone)]
pub struct TestOptions {
    pub build: BuildOptions,
    pub filter: Option<String>,
    pub timeout_secs: u64,
    /// Launch Factorio with a window (`--load-scenario`) instead of headless.
    pub gui: bool,
}

/// Run discovered Factorio simulations and print a cargo-test-style report.
///
/// Returns `Ok(())` only when every selected test passed.
pub fn run_tests(project_root: &Path, options: &TestOptions) -> CliResult<()> {
    let binary = require_factorio_binary()?;

    if !options.build.skip_typecheck {
        typecheck::cargo_check_tests(project_root)?;
    }

    // Typecheck already ran (or was skipped); avoid a second `cargo check` in build.
    let build_options = options.build.clone().with_skip_typecheck(true);
    build(project_root, &build_options)?;

    let config = Config::load(project_root)?;
    let package = CargoPackage::load(project_root)?;
    let output_dir = project_root.join(&config.output_dir);

    let suite = load_test_suite(project_root, &config)?;
    let mut tests = suite.tests.clone();
    if let Some(filter) = &options.filter {
        tests.retain(|test| test.name.contains(filter));
    }
    if tests.is_empty() {
        return Err(CliError::NoTests);
    }

    let mut filtered_suite = suite;
    filtered_suite.tests.clone_from(&tests);
    let module = filtered_suite.to_module();

    let lua_module_prefix = config.emit.lua_module_prefix.as_deref().unwrap_or("");
    let profile = config.resolve_profile(&options.build.profile);
    let mut generator = options
        .build
        .debug_level
        .or(profile.debug_level)
        .map_or_else(
            || LuaGenerator::with_mod_name(&package.name),
            |level| LuaGenerator::with_mod_name_and_debug(&package.name, level),
        );
    if !lua_module_prefix.is_empty() {
        generator = generator.with_module_prefix(lua_module_prefix);
    }
    generator = generator.with_profile(&options.build.profile);
    let tests_lua = generator.generate_module(&module)?;

    let tests_lua_path = output_dir.join("lua").join("factorio_rs_tests.lua");
    if let Some(parent) = tests_lua_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(&tests_lua_path, tests_lua).map_err(|source| CliError::WriteFile {
        path: tests_lua_path.clone(),
        source,
    })?;

    let control_path = output_dir.join("control.lua");
    let mut control =
        std::fs::read_to_string(&control_path).map_err(|source| CliError::ReadFile {
            path: control_path.clone(),
            source,
        })?;
    control.push('\n');
    control.push_str(&generate_harness_lua(&package.name, &tests));
    std::fs::write(&control_path, control).map_err(|source| CliError::WriteFile {
        path: control_path.clone(),
        source,
    })?;

    let work_dir = project_root.join(".factorio-rs").join("test-run");
    prepare_work_dir(&work_dir, &output_dir, &package)?;

    println!("running {} tests", tests.len());
    if options.gui {
        println!("gui mode: Factorio will stay open after the suite finishes");
    }
    let outcome = launch_and_collect(
        &binary,
        &work_dir,
        &package,
        options.timeout_secs,
        options.gui,
    )?;
    print_report(&tests, &outcome);

    if outcome.timed_out {
        return Err(CliError::TestTimeout {
            timeout_secs: options.timeout_secs,
        });
    }
    if outcome.failed > 0 || !outcome.suite_finished {
        return Err(CliError::TestsFailed);
    }
    Ok(())
}

fn require_factorio_binary() -> CliResult<FactorioLaunchTarget> {
    let target = find_factorio()?;
    match target {
        FactorioLaunchTarget::Binary { .. } => Ok(target),
        FactorioLaunchTarget::Steam => Err(CliError::FactorioBinaryRequired),
    }
}

fn load_test_suite(
    project_root: &Path,
    config: &Config,
) -> CliResult<factorio_frontend::TestSuite> {
    let bindings = crate::bindings::discover_bindings(project_root)?;
    let source_dir = project_root.join(&config.source);
    let sources = collect_sources(&source_dir)?;
    let lint_config = config.lints.resolve()?;
    let lua_module_prefix = config.emit.lua_module_prefix.as_deref().unwrap_or("");
    let parse_options = ParseOptions::new(&lint_config)
        .with_prefix(lua_module_prefix)
        .with_bindings(&bindings);

    let mut diagnostics = Vec::new();
    let suite = match discover_tests(&source_dir, &sources, &parse_options, &mut diagnostics) {
        Ok(suite) => suite,
        Err(err) => {
            if let Some((path, source)) = sources.first() {
                let filename = display_filename(path);
                let _ = eprint_frontend_error(&filename, source, &err);
            }
            return Err(CliError::Frontend(err));
        }
    };
    let mut failed = false;
    if let Some((path, source)) = sources.first() {
        let filename = display_filename(path);
        for diagnostic in &diagnostics {
            let _ = eprint_diagnostic(&filename, source, diagnostic);
            if diagnostic.is_error() {
                failed = true;
            }
        }
    }
    if failed {
        return Err(CliError::Reported);
    }
    if suite.is_empty() {
        return Err(CliError::NoTests);
    }
    Ok(suite)
}

fn collect_sources(source_dir: &Path) -> CliResult<Vec<(PathBuf, String)>> {
    let mut paths = Vec::new();
    collect_rust_paths(source_dir, &mut paths)?;
    paths.sort();
    let mut sources = Vec::new();
    for path in paths {
        let source = std::fs::read_to_string(&path).map_err(|err| CliError::ReadFile {
            path: path.clone(),
            source: err,
        })?;
        sources.push((path, source));
    }
    Ok(sources)
}

fn collect_rust_paths(dir: &Path, out: &mut Vec<PathBuf>) -> CliResult<()> {
    for entry in std::fs::read_dir(dir).map_err(|source| CliError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| CliError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_paths(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
            && path
                .file_name()
                .is_some_and(|name| name != "factorio_exports.rs")
        {
            out.push(path);
        }
    }
    Ok(())
}

fn generate_harness_lua(mod_name: &str, tests: &[factorio_frontend::FactorioTest]) -> String {
    let mut out = String::new();
    out.push_str("-- factorio-rs test harness\n");
    out.push_str("do\n");
    let _ = writeln!(
        out,
        "  local __frs_suite = require(\"__{mod_name}__/lua/factorio_rs_tests\")"
    );
    out.push_str("  local __frs_tests = {\n");
    for test in tests {
        let _ = writeln!(
            out,
            "    {{ name = \"{}\", fn = __frs_suite.{} }},",
            escape_lua_string(&test.name),
            test.lua_name
        );
    }
    out.push_str("  }\n");
    out.push_str(
        r#"  local function __frs_emit(line)
    -- Emit once: localised_print reaches the parent via stdout; the results
    -- file is the durable fallback. Do not also print/log - that triples counts.
    localised_print(line)
  end
  local function __frs_run_suite()
    if storage.__factorio_rs_tests_done then
      return
    end
    storage.__factorio_rs_tests_done = true
    local lines = {}
    local passed = 0
    local failed = 0
    for _, test in ipairs(__frs_tests) do
      local start_line = "FACTORIO_RS_TEST start " .. test.name
      __frs_emit(start_line)
      table.insert(lines, start_line)
      local ok, err = pcall(test.fn)
      local result_line
      if ok then
        result_line = "FACTORIO_RS_TEST ok " .. test.name
        passed = passed + 1
      else
        result_line = "FACTORIO_RS_TEST fail " .. test.name .. " " .. tostring(err)
        failed = failed + 1
      end
      __frs_emit(result_line)
      table.insert(lines, result_line)
    end
    local end_line = "FACTORIO_RS_TEST suite_end " .. tostring(passed) .. " " .. tostring(failed)
    __frs_emit(end_line)
    table.insert(lines, end_line)
    helpers.write_file("factorio-rs-test-results.txt", table.concat(lines, "\n") .. "\n", false)
  end
  script.on_init(__frs_run_suite)
  script.on_nth_tick(1, function()
    script.on_nth_tick(1, nil)
    __frs_run_suite()
  end)
end
"#,
    );
    out
}

fn escape_lua_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn prepare_work_dir(work_dir: &Path, output_dir: &Path, package: &CargoPackage) -> CliResult<()> {
    if work_dir.exists() {
        std::fs::remove_dir_all(work_dir).map_err(|source| CliError::RemoveDir {
            path: work_dir.to_path_buf(),
            source,
        })?;
    }

    let mods_dir = work_dir.join("mods");
    let mod_dest = mods_dir.join(format!("{}_{}", package.name, package.version));
    copy_dir_recursive(output_dir, &mod_dest)?;

    let mod_list = serde_json::json!({
        "mods": [
            { "name": "base", "enabled": true },
            { "name": package.name, "enabled": true },
        ]
    });
    write_json(&mods_dir.join("mod-list.json"), &mod_list)?;

    let server_settings = serde_json::json!({
        "name": "factorio-rs-test",
        "description": "Automated factorio-rs test run",
        "max_players": 0,
        "visibility": { "public": false, "lan": false },
        "autosave_interval": 0,
        "autosave_only_on_server": true,
        "non_blocking_saving": true,
        "game_password": "",
        "require_user_verification": false,
        "auto_pause": false,
        "auto_pause_when_players_connect": false,
    });
    write_json(&work_dir.join("server-settings.json"), &server_settings)?;

    let write_data = work_dir
        .canonicalize()
        .unwrap_or_else(|_| work_dir.to_path_buf());
    let config_ini = format!(
        "; version=13\n\
         [path]\n\
         read-data=__PATH__system-read-data__\n\
         write-data={}\n",
        write_data.display()
    );
    std::fs::write(work_dir.join("config.ini"), config_ini).map_err(|source| {
        CliError::WriteFile {
            path: work_dir.join("config.ini"),
            source,
        }
    })?;

    let scenario_dir = work_dir.join("scenarios").join("factorio-rs-test");
    std::fs::create_dir_all(&scenario_dir).map_err(|source| CliError::CreateDir {
        path: scenario_dir.clone(),
        source,
    })?;
    write_json(
        &scenario_dir.join("description.json"),
        &serde_json::json!({
            "multiplayer-compatible": true,
            "order": "a",
        }),
    )?;
    std::fs::write(
        scenario_dir.join("control.lua"),
        "-- factorio-rs test scenario\n",
    )
    .map_err(|source| CliError::WriteFile {
        path: scenario_dir.join("control.lua"),
        source,
    })?;

    Ok(())
}

fn write_json(path: &Path, value: &serde_json::Value) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let body = serde_json::to_vec_pretty(value)
        .map_err(|source| CliError::InfoJsonSerialize { source })?;
    std::fs::write(path, body).map_err(|source| CliError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> CliResult<()> {
    std::fs::create_dir_all(dest).map_err(|source| CliError::CreateDir {
        path: dest.to_path_buf(),
        source,
    })?;

    for entry in walkdir::WalkDir::new(source) {
        let entry = entry.map_err(|err| CliError::ReadDir {
            path: source.to_path_buf(),
            source: std::io::Error::other(err),
        })?;
        let path = entry.path();
        let relative = path
            .strip_prefix(source)
            .map_err(|_| CliError::InvalidProjectPath {
                path: path.to_path_buf(),
            })?;
        let target = dest.join(relative);

        if path.is_dir() {
            std::fs::create_dir_all(&target).map_err(|source| CliError::CreateDir {
                path: target,
                source,
            })?;
            continue;
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|source| CliError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        std::fs::copy(path, &target).map_err(|source| CliError::WriteFile {
            path: target,
            source,
        })?;
    }

    Ok(())
}

#[derive(Debug, Default)]
struct SuiteOutcome {
    results: Vec<TestResult>,
    passed: u32,
    failed: u32,
    suite_finished: bool,
    timed_out: bool,
}

#[derive(Debug, Clone)]
struct TestResult {
    name: String,
    status: TestStatus,
    message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestStatus {
    Ok,
    Failed,
}

fn launch_and_collect(
    target: &FactorioLaunchTarget,
    work_dir: &Path,
    package: &CargoPackage,
    timeout_secs: u64,
    gui: bool,
) -> CliResult<SuiteOutcome> {
    let mods_dir = work_dir.join("mods");
    let server_settings = work_dir.join("server-settings.json");
    let config_ini = work_dir.join("config.ini");

    let mut command = match target {
        FactorioLaunchTarget::Binary {
            path,
            steam_run: true,
        } => {
            let mut command = Command::new("steam-run");
            command.arg(path);
            command
        }
        FactorioLaunchTarget::Binary {
            path,
            steam_run: false,
        } => Command::new(path),
        FactorioLaunchTarget::Steam => {
            return Err(CliError::FactorioBinaryRequired);
        }
    };

    command
        .arg("--config")
        .arg(&config_ini)
        .arg("--mod-directory")
        .arg(&mods_dir)
        .arg("--map-gen-seed")
        .arg("1")
        .arg("--disable-audio");

    if gui {
        // Singleplayer window - watch placements / inspect the map after.
        command.arg("--load-scenario").arg("factorio-rs-test");
    } else {
        command
            .arg("--start-server-load-scenario")
            .arg("factorio-rs-test")
            .arg("--server-settings")
            .arg(&server_settings);
    }

    command
        .current_dir(work_dir)
        // Keep stdin open
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let _ = package;
    let mut child = command.spawn().map_err(|source| CliError::LaunchFactorio {
        target: target.display(),
        source,
    })?;
    // Hold stdin so Factorio does not observe EOF until we drop it at the end.
    let stdin = child.stdin.take();

    let results_path = work_dir
        .join("script-output")
        .join("factorio-rs-test-results.txt");
    let outcome = read_protocol(&mut child, &results_path, Duration::from_secs(timeout_secs))?;

    if gui && outcome.suite_finished && !outcome.timed_out {
        eprintln!("Suite finished - close Factorio to exit.");
        drop(stdin);
    } else {
        drop(stdin);
        let _ = child.kill();
    }
    let _ = child.wait();
    Ok(outcome)
}

fn read_protocol(
    child: &mut Child,
    results_path: &Path,
    timeout: Duration,
) -> CliResult<SuiteOutcome> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CliError::LaunchFactorio {
            target: "factorio".to_string(),
            source: std::io::Error::other("missing stdout pipe"),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CliError::LaunchFactorio {
            target: "factorio".to_string(),
            source: std::io::Error::other("missing stderr pipe"),
        })?;

    let (tx, rx) = mpsc::channel::<String>();
    spawn_line_reader(stdout, tx.clone());
    spawn_line_reader(stderr, tx);

    let mut outcome = SuiteOutcome::default();
    let deadline = Instant::now() + timeout;

    loop {
        if !outcome.suite_finished
            && let Ok(contents) = std::fs::read_to_string(results_path)
            && contents.contains("FACTORIO_RS_TEST suite_end")
        {
            let mut from_file = SuiteOutcome::default();
            for line in contents.lines() {
                let _ = parse_protocol_line(line, &mut from_file);
            }
            if from_file.suite_finished {
                outcome = from_file;
                break;
            }
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            outcome.timed_out = true;
            break;
        }
        match rx.recv_timeout(Duration::from_millis(100).min(remaining)) {
            Ok(line) => {
                eprintln!("{line}");
                if parse_protocol_line(&line, &mut outcome) && outcome.suite_finished {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
                // Keep polling the results file until timeout / suite_end.
            }
        }
    }

    Ok(outcome)
}

fn spawn_line_reader<R: Read + Send + 'static>(reader: R, tx: mpsc::Sender<String>) {
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

fn parse_protocol_line(line: &str, outcome: &mut SuiteOutcome) -> bool {
    let Some(rest) = line.find(PROTOCOL_PREFIX).map(|idx| &line[idx..]) else {
        return false;
    };
    let rest = rest.trim_start_matches(PROTOCOL_PREFIX).trim_start();
    let mut parts = rest.splitn(3, ' ');
    let Some(kind) = parts.next() else {
        return false;
    };
    match kind {
        "start" => true,
        "ok" => {
            if let Some(name) = parts.next() {
                // Ignore duplicate protocol lines
                if outcome.results.iter().any(|r| r.name == name) {
                    return true;
                }
                outcome.results.push(TestResult {
                    name: name.to_string(),
                    status: TestStatus::Ok,
                    message: None,
                });
                outcome.passed += 1;
            }
            true
        }
        "fail" => {
            let name = parts.next().unwrap_or("unknown").to_string();
            let message = parts.next().map(str::to_string);
            if outcome.results.iter().any(|r| r.name == name) {
                return true;
            }
            outcome.results.push(TestResult {
                name,
                status: TestStatus::Failed,
                message,
            });
            outcome.failed += 1;
            true
        }
        "suite_end" => {
            outcome.suite_finished = true;
            true
        }
        _ => false,
    }
}

fn use_color() -> bool {
    stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn paint(enabled: bool, code: &str, text: &str) -> String {
    if enabled {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn print_report(expected: &[factorio_frontend::FactorioTest], outcome: &SuiteOutcome) {
    let color = use_color();
    let ok_tag = paint(color, "32;1", "[OK]");
    let fail_tag = paint(color, "31;1", "[FAIL]");

    for test in expected {
        let result = outcome
            .results
            .iter()
            .find(|result| result.name == test.name);
        match result {
            Some(TestResult {
                status: TestStatus::Ok,
                ..
            }) => println!("{ok_tag} {}", test.name),
            Some(TestResult {
                status: TestStatus::Failed,
                message,
                ..
            }) => {
                println!("{fail_tag} {}", test.name);
                if let Some(message) = message {
                    println!("       {message}");
                }
            }
            None => println!("{fail_tag} {} (no result)", test.name),
        }
    }

    let failures: Vec<_> = outcome
        .results
        .iter()
        .filter(|result| result.status == TestStatus::Failed)
        .collect();
    if !failures.is_empty() {
        println!("\n{}", paint(color, "31;1", "failures:"));
        println!();
        for failure in failures {
            println!("---- {} ----", paint(color, "31;1", &failure.name));
            if let Some(message) = &failure.message {
                println!("{message}");
            }
            println!();
        }
    }

    let missing = expected
        .iter()
        .filter(|test| {
            outcome
                .results
                .iter()
                .all(|result| result.name != test.name)
        })
        .count();
    let failed = outcome.failed + u32::try_from(missing).unwrap_or(u32::MAX);
    let passed = outcome.passed;
    let ok = failed == 0 && outcome.suite_finished && !outcome.timed_out;
    let status = if ok {
        paint(color, "32;1", "ok")
    } else {
        paint(color, "31;1", "FAILED")
    };
    let passed_s = paint(color, "32", &format!("{passed} passed"));
    let failed_s = if failed == 0 {
        format!("{failed} failed")
    } else {
        paint(color, "31", &format!("{failed} failed"))
    };
    println!("\ntest result: {status}. {passed_s}; {failed_s}; 0 ignored");
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn parses_protocol_lines() {
        let mut outcome = SuiteOutcome::default();
        assert!(parse_protocol_line(
            "FACTORIO_RS_TEST start tests::foo",
            &mut outcome
        ));
        assert!(parse_protocol_line(
            "  FACTORIO_RS_TEST ok tests::foo",
            &mut outcome
        ));
        assert!(parse_protocol_line(
            "FACTORIO_RS_TEST fail tests::bar boom",
            &mut outcome
        ));
        assert!(parse_protocol_line(
            "FACTORIO_RS_TEST suite_end 1 1",
            &mut outcome
        ));
        assert_eq!(outcome.passed, 1);
        assert_eq!(outcome.failed, 1);
        assert!(outcome.suite_finished);
        assert_eq!(outcome.results[1].message.as_deref(), Some("boom"));
    }

    #[test]
    fn harness_includes_test_names() {
        let tests = vec![factorio_frontend::FactorioTest {
            name: "tests::truth".to_string(),
            lua_name: "truth".to_string(),
            function: factorio_ir::function::Function {
                name: "truth".to_string(),
                params: vec![],
                body: factorio_ir::block::Block { statements: vec![] },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
            },
        }];
        let lua = generate_harness_lua("hello_world", &tests);
        assert!(lua.contains("tests::truth"));
        assert!(lua.contains("__frs_suite.truth"));
        assert!(lua.contains("FACTORIO_RS_TEST suite_end"));
        assert!(lua.contains("localised_print"));
        assert!(lua.contains("factorio-rs-test-results.txt"));
    }
}
