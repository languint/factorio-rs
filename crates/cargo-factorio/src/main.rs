use clap::Parser;

mod cli;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::try_parse()?;

    match cli.command {
        cli::Command::Init => {}
        cli::Command::Build => {}
    }

    println!("Hello, world!");

    Ok(())
}
