use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new cargo-factorio project in the current directory.
    Init(InitArgs),
    /// Transpile Rust sources to a loadable Factorio mod directory.
    Build(BuildArgs),
    /// Build and package the mod into a Factorio-ready zip archive.
    Package(PackageArgs),
    /// Build and copy the mod into the Factorio mods directory.
    Install(InstallArgs),
}

#[derive(Debug, Parser)]
#[command(
    name = "factorio",
    bin_name = "cargo factorio",
    about = "Transpile Rust into Lua for Factorio mods",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Parser)]
pub struct InitArgs {
    /// Name of the generated Cargo package.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,

    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct BuildArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Emit Rust source comments in generated Lua for debugging.
    #[arg(long, value_name = "LEVEL")]
    pub debug_level: Option<u8>,

    /// Also create a `{name}_{version}.zip` archive after building.
    #[arg(long)]
    pub package: bool,
}

#[derive(Debug, Parser)]
pub struct PackageArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct InstallArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,
}
