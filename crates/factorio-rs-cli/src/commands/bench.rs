use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use factorio_codegen::LuaGenerator;
use factorio_frontend::{
    ParseOptions, discover_benches, display_filename, eprint_diagnostic, eprint_frontend_error,
};
use factorio_ir::opt::optimize_modules;

use crate::{
    cargo_manifest::CargoPackage,
    commands::build::{BuildOptions, build},
    commands::deploy::{DeployMode, deploy_mod, mod_dest},
    commands::typecheck,
    config::Config,
    error::{CliError, CliResult},
    paths::{FactorioLaunchTarget, find_factorio},
    status::{self, Status},
};

const PROTOCOL_PREFIX: &str = "FACTORIO_RS_BENCH";

/// Options for [`run_benches`].
#[derive(Debug, Clone)]
pub struct BenchOptions {
    pub build: BuildOptions,
    pub filter: Option<String>,
    pub timeout_secs: u64,
    pub gui: bool,
}

/// Run discovered Factorio microbenchmarks and print a report.
///
/// Returns `Ok(())` only when every selected bench produced the expected
/// number of samples and none failed.
#[allow(clippy::too_many_lines)]
pub fn run_benches(project_root: &Path, options: &BenchOptions) -> CliResult<()> {
    let binary = require_factorio_binary()?;

    if !options.build.skip_typecheck {
        typecheck::cargo_check_tests(project_root)?;
    }

    let build_options = options.build.clone().with_skip_typecheck(true);
    build(project_root, &build_options)?;

    let config = Config::load(project_root)?;
    let package = CargoPackage::load(project_root)?;
    let output_dir = project_root.join(&config.output_dir);

    let suite = load_bench_suite(project_root, &config)?;
    let mut benches = suite.benches.clone();
    if let Some(filter) = &options.filter {
        benches.retain(|b| b.name.contains(filter.as_str()));
    }
    if benches.is_empty() {
        return Err(CliError::NoBenches);
    }

    let mut filtered_suite = suite;
    filtered_suite.benches.clone_from(&benches);
    let mut module = filtered_suite.to_module();

    let lua_module_prefix = config.emit.lua_module_prefix.as_deref().unwrap_or("");
    let profile = config.resolve_profile(&options.build.profile);
    if profile.optimize_ir {
        optimize_modules(std::slice::from_mut(&mut module));
    }
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
    let benches_lua = generator.generate_module(&module)?;

    let benches_lua_path = output_dir.join("lua").join("factorio_rs_benches.lua");
    if let Some(parent) = benches_lua_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(&benches_lua_path, benches_lua).map_err(|source| CliError::WriteFile {
        path: benches_lua_path.clone(),
        source,
    })?;

    let control_path = output_dir.join("control.lua");
    let mut control =
        std::fs::read_to_string(&control_path).map_err(|source| CliError::ReadFile {
            path: control_path.clone(),
            source,
        })?;
    control.push('\n');
    control.push_str(&generate_bench_harness_lua(&package.name, &benches));
    std::fs::write(&control_path, control).map_err(|source| CliError::WriteFile {
        path: control_path.clone(),
        source,
    })?;

    let work_dir = project_root.join(".factorio-rs").join("test-run");
    ensure_work_dir(&work_dir, &output_dir, &package)?;

    status::status(
        Status::Running,
        format!(
            "{} bench{}",
            benches.len(),
            if benches.len() == 1 { "" } else { "es" }
        ),
    );

    if options.gui {
        status::status(
            Status::Note,
            "gui mode: Factorio will stay open after the suite finishes",
        );
    }

    let outcome = launch_and_collect(&binary, &work_dir, options.timeout_secs, options.gui)?;

    print_bench_report(&benches, &outcome);
    finish_outcome(&outcome, options.timeout_secs)
}

fn load_bench_suite(
    project_root: &Path,
    config: &Config,
) -> CliResult<factorio_frontend::BenchSuite> {
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
    let suite = match discover_benches(&source_dir, &sources, &parse_options, &mut diagnostics) {
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
        return Err(CliError::NoBenches);
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

fn escape_lua_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn generate_bench_harness_lua(
    mod_name: &str,
    benches: &[factorio_frontend::FactorioBench],
) -> String {
    let mut out = String::new();
    out.push_str("-- factorio-rs bench harness\n");
    out.push_str("do\n");
    let _ = writeln!(
        out,
        "  local __frs_suite = require(\"__{mod_name}__/lua/factorio_rs_benches\")"
    );
    out.push_str("  local __frs_benches = {\n");
    for bench in benches {
        let _ = writeln!(
            out,
            "    {{ name = \"{}\", fn = __frs_suite.{}, iterations = {} }},",
            escape_lua_string(&bench.name),
            bench.lua_name,
            bench.iterations,
        );
    }
    out.push_str("  }\n");
    out.push_str(
        r#"  local function __frs_emit(line)
    localised_print(line)
    helpers.write_file("factorio-rs-bench-results.txt", line .. "\n", true)
  end
  local __frs_ran = false
  local function __frs_run_all()
    if __frs_ran then return end
    __frs_ran = true
    helpers.write_file("factorio-rs-bench-results.txt", "", false)
    for _, bench in ipairs(__frs_benches) do
      __frs_emit("FACTORIO_RS_BENCH start " .. bench.name .. " " .. tostring(bench.iterations))
      local bench_failed = false
      for i = 1, bench.iterations do
        local p = helpers.create_profiler()
        local ok, err = pcall(bench.fn)
        p.stop()
        if not ok then
          __frs_emit("FACTORIO_RS_BENCH fail " .. bench.name .. " " .. tostring(err))
          bench_failed = true
          break
        end
        -- LocalisedString so Factorio expands the profiler to `Duration: …ms`.
        local sample = {"", "FACTORIO_RS_BENCH sample ", bench.name, " ", tostring(i), " ", p}
        localised_print(sample)
        helpers.write_file("factorio-rs-bench-results.txt", sample, true)
        helpers.write_file("factorio-rs-bench-results.txt", "\n", true)
      end
      if not bench_failed then
        __frs_emit("FACTORIO_RS_BENCH done " .. bench.name)
      end
    end
    __frs_emit("FACTORIO_RS_BENCH suite_end")
  end
  script.on_init(function()
    __frs_run_all()
  end)
  script.on_nth_tick(1, function()
    __frs_run_all()
    script.on_nth_tick(1, nil)
  end)
end
"#,
    );
    out
}

fn ensure_work_dir(work_dir: &Path, output_dir: &Path, package: &CargoPackage) -> CliResult<()> {
    if work_dir.join("config.ini").is_file() {
        update_mod_in_work_dir(work_dir, output_dir, package)?;
        return Ok(());
    }
    prepare_work_dir(work_dir, output_dir, package)
}

fn update_mod_in_work_dir(
    work_dir: &Path,
    output_dir: &Path,
    package: &CargoPackage,
) -> CliResult<()> {
    let mods_dir = work_dir.join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|source| CliError::CreateDir {
        path: mods_dir.clone(),
        source,
    })?;
    let mod_dest_path = mod_dest(&mods_dir, &package.name, &package.version);
    deploy_mod(output_dir, &mod_dest_path, DeployMode::Copy)?;

    let mod_list = serde_json::json!({
        "mods": [
            { "name": "base", "enabled": true },
            { "name": package.name, "enabled": true },
        ]
    });
    write_json(&mods_dir.join("mod-list.json"), &mod_list)?;
    Ok(())
}

fn prepare_work_dir(work_dir: &Path, output_dir: &Path, package: &CargoPackage) -> CliResult<()> {
    if work_dir.exists() {
        std::fs::remove_dir_all(work_dir).map_err(|source| CliError::RemoveDir {
            path: work_dir.to_path_buf(),
            source,
        })?;
    }

    let mods_dir = work_dir.join("mods");
    let mod_path = mod_dest(&mods_dir, &package.name, &package.version);
    deploy_mod(output_dir, &mod_path, DeployMode::Copy)?;

    let mod_list = serde_json::json!({
        "mods": [
            { "name": "base", "enabled": true },
            { "name": package.name, "enabled": true },
        ]
    });
    write_json(&mods_dir.join("mod-list.json"), &mod_list)?;

    let server_settings = serde_json::json!({
        "name": "factorio-rs-bench",
        "description": "Automated factorio-rs bench run",
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

fn require_factorio_binary() -> CliResult<FactorioLaunchTarget> {
    let target = find_factorio()?;
    match target {
        FactorioLaunchTarget::Binary { .. } => Ok(target),
        FactorioLaunchTarget::Steam => Err(CliError::FactorioBinaryRequired),
    }
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

fn launch_and_collect(
    target: &FactorioLaunchTarget,
    work_dir: &Path,
    timeout_secs: u64,
    gui: bool,
) -> CliResult<BenchOutcome> {
    // Absolute paths: relative `--config` breaks when Factorio/`steam-run` resolves
    // against `current_dir` (e.g. `--manifest-path examples/...`).
    let work_dir = work_dir
        .canonicalize()
        .unwrap_or_else(|_| work_dir.to_path_buf());
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
        command.arg("--load-scenario").arg("factorio-rs-test");
    } else {
        command
            .arg("--start-server-load-scenario")
            .arg("factorio-rs-test")
            .arg("--server-settings")
            .arg(&server_settings);
    }

    command
        .current_dir(&work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|source| CliError::LaunchFactorio {
        target: target.display(),
        source,
    })?;
    let stdin = child.stdin.take();

    let results_path = work_dir
        .join("script-output")
        .join("factorio-rs-bench-results.txt");
    // Avoid treating a previous run's suite_end as completion.
    let _ = std::fs::remove_file(&results_path);
    let outcome = read_protocol(&mut child, &results_path, Duration::from_secs(timeout_secs))?;

    if gui && outcome.suite_finished && !outcome.timed_out {
        status::status_err(
            Status::Note,
            "bench suite finished - close Factorio to exit",
        );
        drop(stdin);
    } else {
        drop(stdin);
        let _ = child.kill();
    }
    let _ = child.wait();
    Ok(outcome)
}

#[derive(Debug, Default)]
struct BenchOutcome {
    /// Accumulated samples per bench name (milliseconds; Factorio always reports ms).
    samples: HashMap<String, Vec<f64>>,
    /// Failure message per bench name.
    failures: HashMap<String, String>,
    /// Benches that emitted `done`.
    done: std::collections::HashSet<String>,
    suite_finished: bool,
    timed_out: bool,
}

/// Display unit chosen from the mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeUnit {
    Ns,
    Us,
    Ms,
    S,
}

impl TimeUnit {
    const fn label(self) -> &'static str {
        match self {
            Self::Ns => "ns",
            Self::Us => "µs",
            Self::Ms => "ms",
            Self::S => "s",
        }
    }

    /// Pick a unit so the mean is typically in `[1, 1000)`.
    fn from_mean_ms(mean_ms: f64) -> Self {
        let abs = mean_ms.abs();
        if abs < 1e-3 {
            Self::Ns // < 1 µs
        } else if abs < 1.0 {
            Self::Us // < 1 ms
        } else if abs < 1_000.0 {
            Self::Ms // < 1 s
        } else {
            Self::S
        }
    }

    fn ms(self, ms: f64) -> f64 {
        match self {
            Self::Ns => ms * 1_000_000.0,
            Self::Us => ms * 1_000.0,
            Self::Ms => ms,
            Self::S => ms / 1_000.0,
        }
    }
}

/// Format a value already converted into `unit` with readable precision.
fn format_scaled(value: f64, unit: TimeUnit) -> String {
    let abs = value.abs();
    let precision = if abs >= 100.0 {
        1
    } else if abs >= 10.0 {
        2
    } else {
        3
    };
    let body = trim_float_zeros(&format!("{value:.precision$}"));
    format!("{body} {}", unit.label())
}

/// Drop trailing zeros after the decimal (`28.10` → `28.1`, `25.00` → `25`).
fn trim_float_zeros(raw: &str) -> String {
    if !raw.contains('.') {
        return raw.to_string();
    }
    let trimmed = raw.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        raw.to_string()
    } else {
        trimmed.to_string()
    }
}

fn format_time_stats(min_ms: f64, mean_ms: f64, max_ms: f64, stddev_ms: f64) -> String {
    let unit = TimeUnit::from_mean_ms(mean_ms);
    let lo = format_scaled(unit.ms(min_ms), unit);
    let mid = format_scaled(unit.ms(mean_ms), unit);
    let hi = format_scaled(unit.ms(max_ms), unit);
    let sd = format_scaled(unit.ms(stddev_ms), unit);
    format!("time: [{lo} {mid} {hi}] ±{sd}")
}

fn read_protocol(
    child: &mut Child,
    results_path: &Path,
    timeout: Duration,
) -> CliResult<BenchOutcome> {
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

    let mut outcome = BenchOutcome::default();
    let deadline = Instant::now() + timeout;

    loop {
        if !outcome.suite_finished
            && let Ok(contents) = std::fs::read_to_string(results_path)
            && contents.contains("FACTORIO_RS_BENCH suite_end")
        {
            let mut from_file = BenchOutcome::default();
            for line in contents.lines() {
                parse_protocol_line(line, &mut from_file);
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
                parse_protocol_line(&line, &mut outcome);
                if outcome.suite_finished {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {}
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

/// Parse a single stdout line and update `outcome`.
///
/// Sample lines look like:
///   `FACTORIO_RS_BENCH sample control::foo 1 Duration: 12.345ms`
/// Factorio expands the profiler `LocalisedString` into that format.
fn parse_protocol_line(line: &str, outcome: &mut BenchOutcome) {
    let Some(idx) = line.find(PROTOCOL_PREFIX) else {
        return;
    };
    let rest = line[idx..].trim_start_matches(PROTOCOL_PREFIX).trim_start();
    let mut parts = rest.splitn(3, ' ');
    let Some(kind) = parts.next() else {
        return;
    };
    match kind {
        "start" | "done" => {
            if kind == "done"
                && let Some(name) = parts.next()
            {
                outcome.done.insert(name.to_string());
            }
        }
        "fail" => {
            let name = parts.next().unwrap_or("unknown").to_string();
            let message = parts.next().unwrap_or("(no message)").to_string();
            outcome.failures.entry(name).or_insert(message);
        }
        "sample" => {
            let mut inner = rest
                .trim_start_matches("sample")
                .trim_start()
                .splitn(3, ' ');
            let name = inner.next().unwrap_or("").to_string();
            let _index = inner.next(); // 1-based index, not needed
            let duration_text = inner.next().unwrap_or("");
            if let Some(ms) = parse_duration_ms(duration_text) {
                outcome.samples.entry(name).or_default().push(ms);
            }
        }
        "suite_end" => {
            outcome.suite_finished = true;
        }
        _ => {}
    }
}

/// Extract a millisecond value from a profiler duration string.
fn parse_duration_ms(text: &str) -> Option<f64> {
    let after = text.find("Duration:")?.checked_add("Duration:".len())?;
    let trimmed = text[after..].trim_start();
    // Parse the float prefix.
    let end = trimmed
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(trimmed.len());
    let num_str = &trimmed[..end];
    num_str.parse::<f64>().ok()
}

#[allow(clippy::as_conversions, clippy::cast_precision_loss)]
fn mean(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.iter().sum::<f64>() / samples.len() as f64
}

fn min_max(samples: &[f64]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for &v in samples {
        min = min.min(v);
        max = max.max(v);
    }
    if samples.is_empty() {
        (0.0, 0.0)
    } else {
        (min, max)
    }
}

/// Sample standard deviation (n−1). Zero when fewer than two samples.
fn stddev(samples: &[f64]) -> f64 {
    let n = samples.len();
    if n < 2 {
        return 0.0;
    }
    let avg = mean(samples);
    #[allow(clippy::as_conversions, clippy::cast_precision_loss)]
    let variance = samples
        .iter()
        .map(|&v| {
            let d = v - avg;
            d * d
        })
        .sum::<f64>()
        / (n - 1) as f64;
    variance.sqrt()
}

fn finish_outcome(outcome: &BenchOutcome, timeout_secs: u64) -> CliResult<()> {
    if outcome.timed_out {
        return Err(CliError::BenchTimeout { timeout_secs });
    }
    if !outcome.suite_finished || !outcome.failures.is_empty() {
        return Err(CliError::BenchesFailed);
    }
    Ok(())
}

fn print_bench_report(expected: &[factorio_frontend::FactorioBench], outcome: &BenchOutcome) {
    println!(
        "\nrunning {} bench{}",
        expected.len(),
        if expected.len() == 1 { "" } else { "es" }
    );

    let mut any_failed = false;

    for bench in expected {
        if let Some(msg) = outcome.failures.get(&bench.name) {
            println!("bench {} ... FAILED: {msg}", bench.name);
            any_failed = true;
        } else {
            #[allow(clippy::pedantic)]
            let samples = outcome
                .samples
                .get(&bench.name)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            #[allow(clippy::as_conversions)]
            let incomplete = samples.len() < bench.iterations as usize;
            if samples.is_empty() {
                println!("bench {} ... FAILED: (no samples)", bench.name);
                any_failed = true;
            } else if incomplete && !outcome.timed_out {
                println!(
                    "bench {} ... FAILED: (incomplete samples: {}/{})",
                    bench.name,
                    samples.len(),
                    bench.iterations
                );
                any_failed = true;
            } else {
                let (lo, hi) = min_max(samples);
                let avg = mean(samples);
                let sd = stddev(samples);
                let stats = format_time_stats(lo, avg, hi, sd);
                if incomplete {
                    println!(
                        "bench {} ... {stats} (incomplete: {}/{})",
                        bench.name,
                        samples.len(),
                        bench.iterations
                    );
                } else {
                    println!("bench {} ... {stats}", bench.name);
                }
            }
        }
    }

    if outcome.timed_out {
        println!(
            "\nbench suite timed out — raise `--timeout` or lower `iterations` \
             (wall clock ≈ sum of body time × iterations)"
        );
    } else if any_failed {
        println!("\nbench result: FAILED");
    } else {
        println!("\nbench result: ok");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    #[test]
    fn parse_sample_line_with_duration() {
        let mut outcome = BenchOutcome::default();
        parse_protocol_line(
            "FACTORIO_RS_BENCH sample control::array_indexing 1 Duration: 12.345ms",
            &mut outcome,
        );
        let samples = outcome.samples.get("control::array_indexing").unwrap();
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - 12.345).abs() < 1e-9, "got {}", samples[0]);
    }

    #[test]
    fn parse_duration_ms_basic() {
        assert!((parse_duration_ms("Duration: 12.345ms").unwrap() - 12.345).abs() < 1e-9);
        assert!((parse_duration_ms("Duration: 0.001ms").unwrap() - 0.001).abs() < 1e-9);
        assert!((parse_duration_ms("Duration: 1234ms").unwrap() - 1234.0).abs() < 1e-9);
        assert!(parse_duration_ms("no duration here").is_none());
    }

    #[test]
    fn parse_start_done_fail_suite_end() {
        let mut outcome = BenchOutcome::default();
        parse_protocol_line("FACTORIO_RS_BENCH start control::foo 3", &mut outcome);
        parse_protocol_line(
            "FACTORIO_RS_BENCH sample control::foo 1 Duration: 1.0ms",
            &mut outcome,
        );
        parse_protocol_line(
            "FACTORIO_RS_BENCH sample control::foo 2 Duration: 2.0ms",
            &mut outcome,
        );
        parse_protocol_line(
            "FACTORIO_RS_BENCH sample control::foo 3 Duration: 3.0ms",
            &mut outcome,
        );
        parse_protocol_line("FACTORIO_RS_BENCH done control::foo", &mut outcome);
        parse_protocol_line("FACTORIO_RS_BENCH fail control::bar boom", &mut outcome);
        parse_protocol_line("FACTORIO_RS_BENCH suite_end", &mut outcome);

        let foo_samples = outcome.samples.get("control::foo").unwrap();
        assert_eq!(foo_samples.len(), 3);
        assert!(outcome.done.contains("control::foo"));
        assert_eq!(
            outcome.failures.get("control::bar").map(String::as_str),
            Some("boom")
        );
        assert!(outcome.suite_finished);
    }

    #[test]
    fn mean_calculation() {
        assert!((mean(&[1.0, 2.0, 3.0]) - 2.0).abs() < 1e-9);
        assert!((mean(&[10.0]) - 10.0).abs() < 1e-9);
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn min_max_and_stddev() {
        assert_eq!(min_max(&[1.0, 2.0, 3.0]), (1.0, 3.0));
        assert_eq!(min_max(&[10.0]), (10.0, 10.0));
        assert_eq!(min_max(&[]), (0.0, 0.0));
        assert_eq!(stddev(&[10.0]), 0.0);
        assert_eq!(stddev(&[]), 0.0);
        // Sample stddev of [1, 2, 3]: mean=2, variance=((1)+(0)+(1))/2 = 1
        let sd = stddev(&[1.0, 2.0, 3.0]);
        assert!((sd - 1.0).abs() < 1e-9, "got {sd}");
    }

    #[test]
    fn scales_duration_units() {
        assert_eq!(TimeUnit::from_mean_ms(0.000_4), TimeUnit::Ns);
        assert_eq!(TimeUnit::from_mean_ms(0.25), TimeUnit::Us);
        assert_eq!(TimeUnit::from_mean_ms(12.0), TimeUnit::Ms);
        assert_eq!(TimeUnit::from_mean_ms(1500.0), TimeUnit::S);

        assert_eq!(
            format_time_stats(0.02, 0.025, 0.03, 0.005),
            "time: [20 µs 25 µs 30 µs] ±5 µs"
        );
        assert_eq!(
            format_time_stats(28.1, 30.7, 33.5, 2.7),
            "time: [28.1 ms 30.7 ms 33.5 ms] ±2.7 ms"
        );
        assert_eq!(
            format_time_stats(900.0, 1200.0, 1500.0, 300.0),
            "time: [0.9 s 1.2 s 1.5 s] ±0.3 s"
        );
        assert_eq!(
            format_time_stats(0.000_2, 0.000_4, 0.000_6, 0.000_1),
            "time: [200 ns 400 ns 600 ns] ±100 ns"
        );
    }

    #[test]
    fn harness_includes_bench_names_and_iterations() {
        let benches = vec![factorio_frontend::FactorioBench {
            name: "control::foo".to_string(),
            lua_name: "foo".to_string(),
            iterations: 5,
            function: factorio_ir::function::Function {
                name: "foo".to_string(),
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
        let lua = generate_bench_harness_lua("hello_world", &benches);
        assert!(lua.contains("control::foo"), "missing bench name");
        assert!(lua.contains("iterations = 5"), "missing iterations");
        assert!(
            lua.contains("FACTORIO_RS_BENCH suite_end"),
            "missing suite_end"
        );
        assert!(lua.contains("helpers.create_profiler"), "missing profiler");
        assert!(lua.contains("factorio_rs_benches"), "missing require");
    }
}
