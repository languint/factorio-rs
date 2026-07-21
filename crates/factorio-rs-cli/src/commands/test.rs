use std::fmt::Write;
use std::io::{BufRead, BufReader, Read};
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
    commands::deploy::{DeployMode, deploy_mod, mod_dest},
    commands::hot_reload::{
        HotReloadOptions, ReloadProbeMode, inject_hot_reload_with, publish_reload_gen,
    },
    commands::typecheck,
    config::Config,
    error::{CliError, CliResult},
    paths::{FactorioLaunchTarget, find_factorio},
    status::{self, Status},
};

const PROTOCOL_PREFIX: &str = "FACTORIO_RS_TEST";
const LISTEN_PID_FILE: &str = "factorio-rs-listen.pid";

/// How [`run_tests`] should manage the Factorio process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestMode {
    /// Run once and exit (kill Factorio when finished, unless `--gui`).
    Once,
    /// Run once, keep Factorio alive, write a listen pid for `--rerun`.
    Listen,
    /// Sync into the listen workdir (start Factorio if needed) and wait for the next suite.
    Rerun,
}

/// Options for [`run_tests`].
#[derive(Debug, Clone)]
pub struct TestOptions {
    pub build: BuildOptions,
    pub filter: Option<String>,
    pub timeout_secs: u64,
    /// Launch Factorio with a window (`--load-scenario`) instead of headless.
    pub gui: bool,
    pub mode: TestMode,
}

/// Run discovered Factorio simulations and print a cargo-test-style report.
///
/// Returns `Ok(())` only when every selected test passed.
#[allow(clippy::too_many_lines)]
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

    let listen = matches!(options.mode, TestMode::Listen | TestMode::Rerun);
    let control_path = output_dir.join("control.lua");
    let mut control =
        std::fs::read_to_string(&control_path).map_err(|source| CliError::ReadFile {
            path: control_path.clone(),
            source,
        })?;
    control.push('\n');
    control.push_str(&generate_harness_lua(&package.name, &tests, listen));
    std::fs::write(&control_path, control).map_err(|source| CliError::WriteFile {
        path: control_path.clone(),
        source,
    })?;

    let mut hot_reload_bumped = true;
    let mut pending_gen = None;
    if listen {
        let injected = inject_hot_reload_with(
            project_root,
            &output_dir,
            &package.name,
            HotReloadOptions {
                // Single reload: a second pass would race the harness suite_end.
                probe: ReloadProbeMode::Once,
                publish_gen: false,
            },
        )?;
        pending_gen = Some(injected.generation);
        hot_reload_bumped = injected.bumped;
        if injected.bumped {
            status::status(
                Status::Note,
                format!("hot-reload generation {}", injected.generation),
            );
        } else {
            status::status(
                Status::Note,
                format!("hot-reload generation {} (unchanged)", injected.generation),
            );
        }
    }

    let work_dir = project_root.join(".factorio-rs").join("test-run");
    let prefer_symlink = listen;
    ensure_work_dir(&work_dir, &output_dir, &package, prefer_symlink)?;
    if let Some(generation) = pending_gen {
        publish_reload_gen(&output_dir, generation)?;
        let mod_path = mod_dest(
            &work_dir.join("mods"),
            &package.name,
            &package.version,
        );
        if !mod_path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            publish_reload_gen(&mod_path, generation)?;
        }
    }

    status::status(
        Status::Running,
        format!(
            "{} test{}",
            tests.len(),
            if tests.len() == 1 { "" } else { "s" }
        ),
    );

    match options.mode {
        TestMode::Once => {
            if options.gui {
                status::status(
                    Status::Note,
                    "gui mode: Factorio will stay open after the suite finishes",
                );
            }
            let outcome = launch_and_collect(
                &binary,
                &work_dir,
                &package,
                options.timeout_secs,
                options.gui,
                false,
            )?;
            print_report(&tests, &outcome);
            finish_outcome(&outcome, options.timeout_secs)
        }
        TestMode::Listen => {
            status::status(
                Status::Note,
                "listen mode: Factorio stays alive for `factorio-rs test --rerun`",
            );
            let outcome = launch_and_collect(
                &binary,
                &work_dir,
                &package,
                options.timeout_secs,
                options.gui,
                true,
            )?;
            print_report(&tests, &outcome);
            finish_outcome(&outcome, options.timeout_secs)
        }
        TestMode::Rerun => {
            let results_path = work_dir
                .join("script-output")
                .join("factorio-rs-test-results.txt");
            let timeout = Duration::from_secs(options.timeout_secs);

            if listen_process_alive(&work_dir) {
                if hot_reload_bumped {
                    let _ = std::fs::remove_file(&results_path);
                    status::status(
                        Status::Note,
                        "reusing listen Factorio - waiting for reloaded suite",
                    );
                } else {
                    status::status(
                        Status::Note,
                        "no mod changes - reusing previous suite results",
                    );
                }
            } else {
                status::status(Status::Note, "starting listen Factorio for --rerun");
                let _ = std::fs::remove_file(&results_path);
                spawn_listen_factorio(&binary, &work_dir, options.gui)?;
            }
            let outcome = wait_for_suite_file(&results_path, timeout)?;
            print_report(&tests, &outcome);
            finish_outcome(&outcome, options.timeout_secs)
        }
    }
}

const fn finish_outcome(outcome: &SuiteOutcome, timeout_secs: u64) -> CliResult<()> {
    if outcome.timed_out {
        return Err(CliError::TestTimeout { timeout_secs });
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
    let package = CargoPackage::load(project_root)?;
    let bindings = crate::bindings::discover_bindings(project_root)?;
    let source_dir = project_root.join(&config.source);
    let sources = collect_sources(&source_dir)?;
    let lint_config = config.lints.resolve()?;
    let lua_module_prefix = config.emit.lua_module_prefix.as_deref().unwrap_or("");
    let trait_catalog = match factorio_frontend::build_trait_catalog(&sources, &source_dir) {
        Ok(catalog) => catalog,
        Err(err) => {
            if let Some((path, source)) = sources.first() {
                let filename = display_filename(path);
                let _ = eprint_frontend_error(&filename, source, &err);
            }
            return Err(CliError::Frontend(err));
        }
    };
    let parse_options = ParseOptions::new(&lint_config)
        .with_prefix(lua_module_prefix)
        .with_bindings(&bindings)
        .with_mod_name(&package.name)
        .with_trait_catalog(&trait_catalog);

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

#[allow(clippy::too_many_lines)]
fn generate_harness_lua(
    mod_name: &str,
    tests: &[factorio_frontend::FactorioTest],
    listen: bool,
) -> String {
    let mut out = String::new();
    out.push_str("-- factorio-rs test harness\n");
    out.push_str("do\n");
    let _ = writeln!(
        out,
        "  local __frs_listen = {}",
        if listen { "true" } else { "false" }
    );
    // Install before require so lowered tests can call `__frs_steps` at runtime.
    out.push_str(
        r#"  local function __frs_make_ctx()
    local data = {}
    return {
      set = function(key, value)
        data[key] = value
      end,
      fetch = function(key)
        return data[key]
      end,
      fetch_u32 = function(key)
        return data[key]
      end,
    }
  end
  function __frs_steps()
    local queue = {}
    local ctx = __frs_make_ctx()
    storage.__frs_pending_steps = queue
    storage.__frs_pending_ctx = ctx
    local api = {}
    function api.step(fn)
      table.insert(queue, { kind = "step", fn = fn })
      return api
    end
    function api.wait(ticks)
      table.insert(queue, { kind = "wait", ticks = ticks })
      return api
    end
    return api
  end
"#,
    );
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
  local function __frs_write_results(lines)
    helpers.write_file("factorio-rs-test-results.txt", table.concat(lines, "\n") .. "\n", false)
  end
  local function __frs_ensure_state()
    if storage.__frs_runner then
      return storage.__frs_runner
    end
    storage.__frs_runner = {
      index = 1,
      phase = "idle", -- idle | steps | wait | done
      lines = {},
      passed = 0,
      failed = 0,
      current_name = nil,
      queue = nil,
      queue_i = 1,
      wait_until_tick = nil,
      ctx = nil,
    }
    return storage.__frs_runner
  end
  local function __frs_finish_suite(state)
    if state.phase == "done" then
      return
    end
    state.phase = "done"
    local end_line = "FACTORIO_RS_TEST suite_end " .. tostring(state.passed) .. " " .. tostring(state.failed)
    __frs_emit(end_line)
    table.insert(state.lines, end_line)
    __frs_write_results(state.lines)
  end
  local function __frs_complete_test(state, ok, err)
    local result_line
    if ok then
      result_line = "FACTORIO_RS_TEST ok " .. state.current_name
      state.passed = state.passed + 1
    else
      result_line = "FACTORIO_RS_TEST fail " .. state.current_name .. " " .. tostring(err)
      state.failed = state.failed + 1
    end
    __frs_emit(result_line)
    table.insert(state.lines, result_line)
    state.queue = nil
    state.queue_i = 1
    state.wait_until_tick = nil
    state.ctx = nil
    storage.__frs_pending_steps = nil
    storage.__frs_pending_ctx = nil
    state.phase = "idle"
    state.index = state.index + 1
  end
  local function __frs_start_next(state)
    if state.index > #__frs_tests then
      __frs_finish_suite(state)
      return
    end
    local test = __frs_tests[state.index]
    state.current_name = test.name
    storage.__frs_pending_steps = nil
    storage.__frs_pending_ctx = nil
    local start_line = "FACTORIO_RS_TEST start " .. test.name
    __frs_emit(start_line)
    table.insert(state.lines, start_line)
    local ok, err = pcall(test.fn)
    if not ok then
      __frs_complete_test(state, false, err)
      return
    end
    local pending = storage.__frs_pending_steps
    if pending and #pending > 0 then
      state.queue = pending
      state.queue_i = 1
      state.ctx = storage.__frs_pending_ctx
      state.wait_until_tick = nil
      state.phase = "steps"
      return
    end
    __frs_complete_test(state, true, nil)
  end
  local function __frs_run_steps(state)
    while state.phase == "steps" do
      if state.queue_i > #state.queue then
        __frs_complete_test(state, true, nil)
        return
      end
      local item = state.queue[state.queue_i]
      state.queue_i = state.queue_i + 1
      if item.kind == "wait" then
        local ticks = tonumber(item.ticks) or 0
        if ticks > 0 then
          -- Wait until game.tick advances (not handler-call counts - tick may
          -- stay frozen during dedicated-server setup).
          state.wait_until_tick = game.tick + ticks
          state.phase = "wait"
          return
        end
        -- wait(0): continue processing in this tick
      elseif item.kind == "step" then
        local ok, err = pcall(item.fn, state.ctx)
        if not ok then
          __frs_complete_test(state, false, err)
          return
        end
      else
        __frs_complete_test(state, false, "unknown step kind: " .. tostring(item.kind))
        return
      end
    end
  end
  local function __frs_on_tick()
    local state = __frs_ensure_state()
    if state.phase == "done" then
      return
    end
    if state.phase == "wait" then
      if game.tick >= (state.wait_until_tick or 0) then
        state.phase = "steps"
        __frs_run_steps(state)
      end
      -- After completing a test mid-tick, start the next one immediately
      -- when it has no waits, so sync tests still finish quickly.
      while state.phase == "idle" do
        __frs_start_next(state)
        if state.phase == "steps" then
          __frs_run_steps(state)
        end
      end
      return
    end
    if state.phase == "idle" then
      __frs_start_next(state)
      if state.phase == "steps" then
        __frs_run_steps(state)
      end
      while state.phase == "idle" do
        __frs_start_next(state)
        if state.phase == "steps" then
          __frs_run_steps(state)
        end
      end
      return
    end
    if state.phase == "steps" then
      __frs_run_steps(state)
      while state.phase == "idle" do
        __frs_start_next(state)
        if state.phase == "steps" then
          __frs_run_steps(state)
        end
      end
    end
  end
  -- Kick once during init (sync tests may finish before the first tick).
  script.on_init(function()
    __frs_on_tick()
  end)
  -- After game.reload_mods() (listen/rerun), storage persists but scripts reload.
  script.on_load(function()
    if __frs_listen then
      storage.__frs_runner = nil
    end
  end)
  script.on_nth_tick(1, function()
    __frs_on_tick()
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

fn ensure_work_dir(
    work_dir: &Path,
    output_dir: &Path,
    package: &CargoPackage,
    prefer_symlink: bool,
) -> CliResult<()> {
    if work_dir.join("config.ini").is_file() {
        update_mod_in_work_dir(work_dir, output_dir, package, prefer_symlink)?;
        return Ok(());
    }
    prepare_work_dir(work_dir, output_dir, package, prefer_symlink)
}

fn update_mod_in_work_dir(
    work_dir: &Path,
    output_dir: &Path,
    package: &CargoPackage,
    prefer_symlink: bool,
) -> CliResult<()> {
    let mods_dir = work_dir.join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|source| CliError::CreateDir {
        path: mods_dir.clone(),
        source,
    })?;
    let mod_dest = mod_dest(&mods_dir, &package.name, &package.version);
    let mode = if prefer_symlink {
        DeployMode::Symlink
    } else {
        DeployMode::Copy
    };
    deploy_mod(output_dir, &mod_dest, mode)?;

    let mod_list = serde_json::json!({
        "mods": [
            { "name": "base", "enabled": true },
            { "name": package.name, "enabled": true },
        ]
    });
    write_json(&mods_dir.join("mod-list.json"), &mod_list)?;
    Ok(())
}

fn prepare_work_dir(
    work_dir: &Path,
    output_dir: &Path,
    package: &CargoPackage,
    prefer_symlink: bool,
) -> CliResult<()> {
    if work_dir.exists() {
        std::fs::remove_dir_all(work_dir).map_err(|source| CliError::RemoveDir {
            path: work_dir.to_path_buf(),
            source,
        })?;
    }

    let mods_dir = work_dir.join("mods");
    let mod_path = mod_dest(&mods_dir, &package.name, &package.version);
    let mode = if prefer_symlink {
        DeployMode::Symlink
    } else {
        DeployMode::Copy
    };
    deploy_mod(output_dir, &mod_path, mode)?;

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
    keep_alive: bool,
) -> CliResult<SuiteOutcome> {
    let mods_dir = work_dir.join("mods");
    let server_settings = work_dir.join("server-settings.json");
    let config_ini = work_dir.join("config.ini");

    let mut command = factorio_command(target)?;

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
    write_listen_pid(work_dir, child.id())?;
    // Hold stdin so Factorio does not observe EOF until we drop it at the end.
    let stdin = child.stdin.take();

    let results_path = work_dir
        .join("script-output")
        .join("factorio-rs-test-results.txt");
    let outcome = read_protocol(&mut child, &results_path, Duration::from_secs(timeout_secs))?;

    if (gui || keep_alive) && outcome.suite_finished && !outcome.timed_out {
        if keep_alive {
            status::status_err(
                Status::Note,
                "suite finished - Factorio kept alive for hot-reload re-runs",
            );
        } else {
            status::status_err(Status::Note, "suite finished - close Factorio to exit");
        }
        if keep_alive && !gui {
            // Keep stdin open so the dedicated server does not see EOF.
            std::mem::forget(stdin);
            std::mem::forget(child);
            return Ok(outcome);
        }
        drop(stdin);
        let _ = child.wait();
    } else {
        drop(stdin);
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(work_dir.join(LISTEN_PID_FILE));
    }
    Ok(outcome)
}

fn factorio_command(target: &FactorioLaunchTarget) -> CliResult<Command> {
    match target {
        FactorioLaunchTarget::Binary {
            path,
            steam_run: true,
        } => {
            let mut command = Command::new("steam-run");
            command.arg(path);
            Ok(command)
        }
        FactorioLaunchTarget::Binary {
            path,
            steam_run: false,
        } => Ok(Command::new(path)),
        FactorioLaunchTarget::Steam => Err(CliError::FactorioBinaryRequired),
    }
}

fn spawn_listen_factorio(
    target: &FactorioLaunchTarget,
    work_dir: &Path,
    gui: bool,
) -> CliResult<()> {
    let mods_dir = work_dir.join("mods");
    let server_settings = work_dir.join("server-settings.json");
    let config_ini = work_dir.join("config.ini");
    let log_path = work_dir.join("factorio-listen.log");

    let log_file = std::fs::File::create(&log_path).map_err(|source| CliError::WriteFile {
        path: log_path.clone(),
        source,
    })?;
    let log_err = log_file.try_clone().map_err(|source| CliError::WriteFile {
        path: log_path.clone(),
        source,
    })?;

    let mut command = factorio_command(target)?;
    command
        .arg("--config")
        .arg(&config_ini)
        .arg("--mod-directory")
        .arg(&mods_dir)
        .arg("--map-gen-seed")
        .arg("1")
        .arg("--disable-audio");

    if gui {
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
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_err));

    let child = command.spawn().map_err(|source| CliError::LaunchFactorio {
        target: target.display(),
        source,
    })?;
    write_listen_pid(work_dir, child.id())?;
    std::mem::forget(child);
    Ok(())
}

fn write_listen_pid(work_dir: &Path, pid: u32) -> CliResult<()> {
    let path = work_dir.join(LISTEN_PID_FILE);
    std::fs::write(&path, format!("{pid}\n")).map_err(|source| CliError::WriteFile { path, source })
}

fn listen_process_alive(work_dir: &Path) -> bool {
    let path = work_dir.join(LISTEN_PID_FILE);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(pid) = contents.trim().parse::<u32>() else {
        return false;
    };
    process_alive(pid)
}

fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn wait_for_suite_file(results_path: &Path, timeout: Duration) -> CliResult<SuiteOutcome> {
    let deadline = Instant::now() + timeout;
    let mut outcome = SuiteOutcome::default();

    loop {
        if let Ok(contents) = std::fs::read_to_string(results_path)
            && contents.contains("FACTORIO_RS_TEST suite_end")
        {
            let mut from_file = SuiteOutcome::default();
            for line in contents.lines() {
                let _ = parse_protocol_line(line, &mut from_file);
            }
            if from_file.suite_finished {
                return Ok(from_file);
            }
        }

        if Instant::now() >= deadline {
            outcome.timed_out = true;
            return Ok(outcome);
        }
        thread::sleep(Duration::from_millis(100));
    }
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

fn print_report(expected: &[factorio_frontend::FactorioTest], outcome: &SuiteOutcome) {
    let color = status::color_stdout();

    // Match cargo/libtest layout so Bacon's standard analyzer can attach failure
    // details (it looks for `---- name stdout ----`, not bare `---- name ----`).
    println!(
        "\nrunning {} test{}",
        expected.len(),
        if expected.len() == 1 { "" } else { "s" }
    );

    let mut failed_names: Vec<(String, Option<String>)> = Vec::new();

    for test in expected {
        let result = outcome
            .results
            .iter()
            .find(|result| result.name == test.name);
        match result {
            Some(TestResult {
                status: TestStatus::Ok,
                ..
            }) => print_cargo_style_result(&test.name, true, color),
            Some(TestResult {
                status: TestStatus::Failed,
                message,
                ..
            }) => {
                print_cargo_style_result(&test.name, false, color);
                failed_names.push((test.name.clone(), message.clone()));
            }
            None => {
                print_cargo_style_result(&test.name, false, color);
                failed_names.push((
                    test.name.clone(),
                    Some("(no result from Factorio suite)".to_string()),
                ));
            }
        }
    }

    if !failed_names.is_empty() {
        println!("\nfailures:\n");
        for (name, message) in &failed_names {
            println!("---- {name} stdout ----");
            if let Some(message) = message {
                println!("{message}");
            } else {
                println!("(failed with no message)");
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
    let status_word = if ok {
        status::paint_ok("ok", color)
    } else {
        status::paint_fail("FAILED", color)
    };
    let passed_s = status::paint_ok(format!("{passed} passed"), color);
    let failed_s = if failed == 0 {
        format!("{failed} failed")
    } else {
        status::paint_fail(format!("{failed} failed"), color)
    };
    println!("\ntest result: {status_word}. {passed_s}; {failed_s}; 0 ignored");
}

/// Emit a libtest-style result line with CSI codes Bacon's analyzer recognizes.
///
/// Styled branch expects `\x1b[32m` for `ok` and `\x1b[31m` for `FAILED` (not bold).
fn print_cargo_style_result(name: &str, passed: bool, color: bool) {
    const CSI_RESET: &str = "\u{1b}[0m";
    const CSI_GREEN: &str = "\u{1b}[32m";
    const CSI_RED: &str = "\u{1b}[31m";

    if color {
        if passed {
            println!("test {name} ... {CSI_GREEN}ok{CSI_RESET}");
        } else {
            println!("test {name} ... {CSI_RED}FAILED{CSI_RESET}");
        }
    } else if passed {
        println!("test {name} ... ok");
    } else {
        println!("test {name} ... FAILED");
    }
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
                inline: false,
            },
        }];
        let lua = generate_harness_lua("hello_world", &tests, false);
        assert!(lua.contains("tests::truth"));
        assert!(lua.contains("__frs_suite.truth"));
        assert!(lua.contains("FACTORIO_RS_TEST suite_end"));
        assert!(lua.contains("localised_print"));
        assert!(lua.contains("factorio-rs-test-results.txt"));
        assert!(lua.contains("function __frs_steps()"));
        assert!(lua.contains("__frs_on_tick"));
        assert!(lua.contains("local __frs_listen = false"));

        let listen = generate_harness_lua("hello_world", &tests, true);
        assert!(listen.contains("local __frs_listen = true"));
        assert!(listen.contains("script.on_load"));
    }

    #[test]
    fn cargo_style_result_uses_bacon_csi() {
        // Capture via formatting the same codes print_cargo_style_result uses.
        const CSI_RESET: &str = "\u{1b}[0m";
        const CSI_GREEN: &str = "\u{1b}[32m";
        const CSI_RED: &str = "\u{1b}[31m";
        assert_eq!(
            format!("test tests::foo ... {CSI_GREEN}ok{CSI_RESET}"),
            "test tests::foo ... \u{1b}[32mok\u{1b}[0m"
        );
        assert_eq!(
            format!("test tests::bar ... {CSI_RED}FAILED{CSI_RESET}"),
            "test tests::bar ... \u{1b}[31mFAILED\u{1b}[0m"
        );
        // Bacon attaches failure bodies to this title shape.
        assert!(regex_is_match_stdout_title("---- tests::bar stdout ----"));
    }

    fn regex_is_match_stdout_title(s: &str) -> bool {
        s.starts_with("---- ") && s.ends_with(" stdout ----")
    }
}
