use clap::Parser;

mod add;
mod api_crate;
mod assets;
mod bindings;
mod build;
mod cargo_manifest;
mod cli;
mod config;
mod error;
mod init;
mod install;
mod locale;
mod manifest;
mod open;
mod package;
mod paths;
mod typecheck;

use build::BuildOptions;
use cli::{AddArgs, BuildArgs, CheckArgs, Cli, Command, InitArgs, InstallArgs, PackageArgs};
use error::project_root;

fn main() -> anyhow::Result<()> {
    let cli = Cli::try_parse()?;

    match cli.command {
        Command::Init(args) => run_init(&args),
        Command::Check(args) => run_check(&args),
        Command::Build(args) => run_build(&args),
        Command::Package(args) => run_package(&args),
        Command::Install(args) => run_install(&args),
        Command::Add(args) => run_add(&args),
        Command::Open => run_open(),
    }
}

fn run_init(args: &InitArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    init::init(&project_root, args.name.as_deref())?;
    println!(
        "Initialized factorio-rs project at `{}`",
        project_root.display()
    );
    Ok(())
}

fn run_check(args: &CheckArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile).with_skip_typecheck(args.skip_typecheck);
    build::check(&project_root, &options)?;
    println!("Check passed");
    Ok(())
}

fn run_build(args: &BuildArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile)
        .with_debug_level(args.debug_level)
        .with_skip_typecheck(args.skip_typecheck);
    let outputs = build::build(&project_root, &options)?;

    for output in outputs {
        println!("Generated `{}`", output.display());
    }

    if args.package {
        let zip_path = package::create_archive(&project_root)?;
        println!("Packaged mod archive `{}`", zip_path.display());
    }

    Ok(())
}

fn run_package(args: &PackageArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile)
        .with_debug_level(args.debug_level)
        .with_skip_typecheck(args.skip_typecheck);
    let zip_path = package::package(&project_root, &options)?;
    println!("Packaged mod archive `{}`", zip_path.display());
    Ok(())
}

fn run_install(args: &InstallArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile)
        .with_debug_level(args.debug_level)
        .with_skip_typecheck(args.skip_typecheck);
    let dest = install::install(&project_root, &options)?;
    println!("Installed mod to `{}`", dest.display());

    if args.open {
        let target = open::open()?;
        println!("Opened Factorio (`{}`)", target.display());
    }

    Ok(())
}

fn run_open() -> anyhow::Result<()> {
    let target = open::open()?;
    println!("Opened Factorio (`{}`)", target.display());
    Ok(())
}

fn run_add(args: &AddArgs) -> anyhow::Result<()> {
    let consumer_root = project_root(args.manifest_path.as_deref())?;
    let result = add::add(&consumer_root, &args.path)?;

    if result.cargo_dep_added {
        println!(
            "Added `{}` = {{ path = \"{}\" }} to Cargo.toml",
            result.crate_name,
            result.dep_path.display()
        );
    } else {
        println!("`{}` already listed in Cargo.toml", result.crate_name);
    }

    for dep in &result.factorio_deps_added {
        println!("Added `{dep}` to Factorio.toml [mod].dependencies");
    }
    if result.factorio_deps_added.is_empty() {
        println!("Factorio.toml dependencies already up to date");
    }

    println!(
        "Use `{}::...` - root remotes call `remote.call`; `{}::shared::...` requires modules.",
        result.rust_crate, result.rust_crate
    );
    if !result.remote_fns.is_empty() {
        println!(
            "Exports: {}",
            result
                .remote_fns
                .iter()
                .map(|name| format!("{}::{name}(...)", result.rust_crate))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Ok(())
}
