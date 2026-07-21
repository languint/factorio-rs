use clap::Parser;

mod api_crate;
mod assets;
mod bindings;
mod cargo_manifest;
mod cli;
mod commands;
mod config;
mod error;
mod locale;
mod manifest;
mod open;
mod paths;
mod progress;
mod status;
mod write_if_changed;

use std::process::ExitCode;
use std::time::Instant;

use cli::{
    AddArgs, BuildArgs, CheckArgs, Cli, Command, InitArgs, InstallArgs, PackageArgs, SyncArgs,
    TestArgs,
};
use commands::build::BuildOptions;
use commands::sync::{SyncOptions, SyncTarget};
use commands::test::TestOptions;
use error::{CliError, project_root};
use status::Status;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            report_error(&err);
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Init(args) => run_init(&args),
        Command::Check(args) => run_check(&args),
        Command::Build(args) => run_build(&args),
        Command::Package(args) => run_package(&args),
        Command::Install(args) => run_install(&args),
        Command::Sync(args) => run_sync(&args),
        Command::Add(args) => run_add(&args),
        Command::Open => run_open(),
        Command::Test(args) => run_test(&args),
    }
}

fn report_error(err: &CliError) {
    // Diagnostics / cargo / the test report already spoke for these.
    if matches!(
        err,
        CliError::Reported | CliError::TypecheckFailed | CliError::TestsFailed
    ) {
        return;
    }
    status::status_err(Status::Error, err.to_string());
}

fn run_init(args: &InitArgs) -> Result<(), CliError> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    commands::init::init(&project_root, args.name.as_deref(), args.bacon)?;
    status::status(
        Status::Created,
        format!(
            "factorio-rs project at {}",
            status::display_path(&project_root)
        ),
    );
    if args.bacon {
        status::status(
            Status::Note,
            "bacon.toml written - try `bacon -j factorio-reload` with Factorio open",
        );
    }
    Ok(())
}

fn run_check(args: &CheckArgs) -> Result<(), CliError> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let package = cargo_manifest::CargoPackage::load(&project_root).ok();
    let name = package
        .as_ref()
        .map_or_else(|| "project".to_string(), |pkg| pkg.name.clone());
    status::status(Status::Checking, format!("{name} (transpile)"));

    let started = Instant::now();
    // Profile does not affect check (no emit/prune); keep default for BuildOptions shape.
    let options = BuildOptions::new("debug").with_skip_typecheck(args.skip_typecheck);
    commands::build::check(&project_root, &options)?;
    status::status(
        Status::Finished,
        format!("check in {}", status::format_elapsed(started.elapsed())),
    );
    Ok(())
}

fn run_build(args: &BuildArgs) -> Result<(), CliError> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile)
        .with_debug_level(args.debug_level)
        .with_skip_typecheck(args.skip_typecheck);
    let _outputs = commands::build::build(&project_root, &options)?;

    if args.package {
        let zip_path = commands::package::create_archive(&project_root)?;
        status::status(
            Status::Packaged,
            format!("mod archive {}", status::display_path(&zip_path)),
        );
    }

    Ok(())
}

fn run_package(args: &PackageArgs) -> Result<(), CliError> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile)
        .with_debug_level(args.debug_level)
        .with_skip_typecheck(args.skip_typecheck);
    let zip_path = commands::package::package(&project_root, &options)?;
    status::status(
        Status::Packaged,
        format!("mod archive {}", status::display_path(&zip_path)),
    );
    Ok(())
}

fn run_install(args: &InstallArgs) -> Result<(), CliError> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile)
        .with_debug_level(args.debug_level)
        .with_skip_typecheck(args.skip_typecheck);
    let dest = commands::install::install(&project_root, &options)?;
    status::status(
        Status::Installed,
        format!("mod to {}", status::display_path(&dest)),
    );

    if args.open {
        let target = open::open()?;
        status::status(Status::Opened, format!("Factorio ({})", target.display()));
    }

    Ok(())
}

fn run_sync(args: &SyncArgs) -> Result<(), CliError> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = SyncOptions {
        build: BuildOptions::new(&args.profile)
            .with_debug_level(args.debug_level)
            .with_skip_typecheck(args.skip_typecheck),
        symlink: args.symlink,
        hot_reload: args.hot_reload,
        target: if args.to_test_run {
            SyncTarget::TestRun
        } else {
            SyncTarget::Mods
        },
    };
    let dest = commands::sync::sync(&project_root, &options)?;
    status::status(
        Status::Installed,
        format!("synced to {}", status::display_path(&dest)),
    );
    Ok(())
}

fn run_open() -> Result<(), CliError> {
    let target = open::open()?;
    status::status(Status::Opened, format!("Factorio ({})", target.display()));
    Ok(())
}

fn run_test(args: &TestArgs) -> Result<(), CliError> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    if args.listen && args.rerun {
        return Err(CliError::InvalidArgs {
            message: "use either --listen or --rerun, not both".to_string(),
        });
    }
    let mode = if args.rerun {
        commands::test::TestMode::Rerun
    } else if args.listen {
        commands::test::TestMode::Listen
    } else {
        commands::test::TestMode::Once
    };
    let options = TestOptions {
        build: BuildOptions::new(&args.profile)
            .with_debug_level(args.debug_level)
            .with_skip_typecheck(args.skip_typecheck),
        filter: args.filter.clone(),
        timeout_secs: args.timeout,
        gui: args.gui,
        mode,
    };
    commands::test::run_tests(&project_root, &options)?;
    Ok(())
}

fn run_add(args: &AddArgs) -> Result<(), CliError> {
    let consumer_root = project_root(args.manifest_path.as_deref())?;
    let result = commands::add::add(&consumer_root, &args.path)?;

    if result.cargo_dep_added {
        status::status(
            Status::Added,
            format!(
                "{} = {{ path = \"{}\" }} to Cargo.toml",
                result.crate_name,
                status::display_path(&result.dep_path)
            ),
        );
    } else {
        status::status(
            Status::Note,
            format!("{} already listed in Cargo.toml", result.crate_name),
        );
    }

    for dep in &result.factorio_deps_added {
        status::status(
            Status::Added,
            format!("{dep} to Factorio.toml [mod].dependencies"),
        );
    }
    if result.factorio_deps_added.is_empty() {
        status::status(
            Status::Note,
            "Factorio.toml dependencies already up to date",
        );
    }

    status::status(
        Status::Note,
        format!(
            "use `{}::...` for remotes; `{}::shared::...` for requireable modules",
            result.rust_crate, result.rust_crate
        ),
    );
    if !result.remote_fns.is_empty() {
        status::status(
            Status::Note,
            format!(
                "exports: {}",
                result
                    .remote_fns
                    .iter()
                    .map(|name| format!("{}::{name}(...)", result.rust_crate))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    }

    Ok(())
}
