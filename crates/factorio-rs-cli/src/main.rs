use clap::Parser;

mod build;
mod cargo_manifest;
mod cli;
mod config;
mod error;
mod init;
mod install;
mod manifest;
mod open;
mod package;
mod paths;

use build::BuildOptions;
use cli::{BuildArgs, Cli, Command, InitArgs, InstallArgs, PackageArgs};
use error::project_root;

fn main() -> anyhow::Result<()> {
    let cli = Cli::try_parse()?;

    match cli.command {
        Command::Init(args) => run_init(&args),
        Command::Build(args) => run_build(&args),
        Command::Package(args) => run_package(&args),
        Command::Install(args) => run_install(&args),
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

fn run_build(args: &BuildArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile).with_debug_level(args.debug_level);
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
    let options = BuildOptions::new(&args.profile).with_debug_level(args.debug_level);
    let zip_path = package::package(&project_root, &options)?;
    println!("Packaged mod archive `{}`", zip_path.display());
    Ok(())
}

fn run_install(args: &InstallArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let options = BuildOptions::new(&args.profile).with_debug_level(args.debug_level);
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
