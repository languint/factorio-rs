use clap::Parser;

mod build;
mod cli;
mod config;
mod error;
mod init;

use cli::{BuildArgs, Cli, Command, InitArgs};
use error::project_root;

fn main() -> anyhow::Result<()> {
    let cli = Cli::try_parse()?;

    match cli.command {
        Command::Init(args) => run_init(&args),
        Command::Build(args) => run_build(&args),
    }
}

fn run_init(args: &InitArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    init::init(&project_root, args.name.as_deref())?;
    println!(
        "Initialized cargo-factorio project at `{}`",
        project_root.display()
    );
    Ok(())
}

fn run_build(args: &BuildArgs) -> anyhow::Result<()> {
    let project_root = project_root(args.manifest_path.as_deref())?;
    let outputs = build::build(&project_root)?;

    for output in outputs {
        println!("Generated `{}`", output.display());
    }

    Ok(())
}
