use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new factorio-rs project in the current directory.
    Init(InitArgs),
    /// Typecheck (`cargo check`) and validate transpile without writing output.
    Check(CheckArgs),
    /// Transpile Rust sources to a loadable Factorio mod directory.
    Build(BuildArgs),
    /// Build and package the mod into a Factorio-ready zip archive.
    Package(PackageArgs),
    /// Build and copy the mod into the Factorio mods directory.
    Install(InstallArgs),
    /// Build and deploy the mod for Bacon / hot-reload (symlink + reload gen).
    Sync(SyncArgs),
    /// Add another factorio-rs library as a Cargo path dependency (+ Factorio.toml).
    Add(AddArgs),
    /// Open Factorio if it is installed on this system.
    Open,
    /// Build the mod, launch Factorio, and run `#[test]` simulations.
    Test(TestArgs),
    /// Build the mod, launch Factorio, and run `#[factorio_rs::bench]` microbenchmarks.
    Bench(BenchArgs),
}

#[derive(Debug, Parser)]
#[command(
    name = "factorio-rs",
    about = "Write Factorio mods in Rust - transpile to loadable Lua mods",
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

    /// Also write a `bacon.toml` with factorio-check / reload / test jobs.
    #[arg(long)]
    pub bacon: bool,
}

#[derive(Debug, Parser)]
pub struct CheckArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Skip `cargo check` and only validate lowering / lints.
    #[arg(long)]
    pub skip_typecheck: bool,
}

#[derive(Debug, Parser)]
pub struct BuildArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Transpile profile from `Factorio.toml` (`debug`, `release`, or custom).
    ///
    /// Defaults to `debug`.
    #[arg(long, value_name = "NAME", default_value = "debug")]
    pub profile: String,

    /// Override the profile's debug comment level in generated Lua.
    #[arg(long, value_name = "LEVEL")]
    pub debug_level: Option<u8>,

    /// Also create a `{name}_{version}.zip` archive after building.
    #[arg(long)]
    pub package: bool,

    /// Skip `cargo check` before transpile (not recommended).
    #[arg(long)]
    pub skip_typecheck: bool,
}

#[derive(Debug, Parser)]
pub struct PackageArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Transpile profile from `Factorio.toml`.
    ///
    /// Defaults to `release`.
    #[arg(long, value_name = "NAME", default_value = "release")]
    pub profile: String,

    /// Override the profile's debug comment level in generated Lua.
    #[arg(long, value_name = "LEVEL")]
    pub debug_level: Option<u8>,

    /// Skip `cargo check` before transpile (not recommended).
    #[arg(long)]
    pub skip_typecheck: bool,
}

#[derive(Debug, Parser)]
pub struct InstallArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Transpile profile from `Factorio.toml`.
    ///
    /// Defaults to `debug`.
    #[arg(long, value_name = "NAME", default_value = "debug")]
    pub profile: String,

    /// Override the profile's debug comment level in generated Lua.
    #[arg(long, value_name = "LEVEL")]
    pub debug_level: Option<u8>,

    /// Open Factorio after installing the mod.
    #[arg(long)]
    pub open: bool,

    /// Skip `cargo check` before transpile (not recommended).
    #[arg(long)]
    pub skip_typecheck: bool,
}

#[derive(Debug, Parser)]
#[allow(clippy::struct_excessive_bools)]
pub struct SyncArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Transpile profile from `Factorio.toml`.
    ///
    /// Defaults to `debug`.
    #[arg(long, value_name = "NAME", default_value = "debug")]
    pub profile: String,

    /// Override the profile's debug comment level in generated Lua.
    #[arg(long, value_name = "LEVEL")]
    pub debug_level: Option<u8>,

    /// Skip `cargo check` before transpile (not recommended).
    #[arg(long)]
    pub skip_typecheck: bool,

    /// Symlink the mods entry to `output_dir`
    #[arg(long)]
    pub symlink: bool,

    /// Inject reload, ping Factorio over UDP.
    #[arg(long)]
    pub hot_reload: bool,

    /// Deploy into `.factorio-rs/test-run/mods/` instead of the user mods dir.
    #[arg(long)]
    pub to_test_run: bool,
}

#[derive(Debug, Parser)]
pub struct AddArgs {
    /// Path to another factorio-rs project (build it first so Cargo metadata exports exist).
    pub path: PathBuf,

    /// Path to the consuming project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,
}

#[derive(Debug, Parser)]
#[allow(clippy::struct_excessive_bools)]
pub struct TestArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Transpile profile from `Factorio.toml`.
    ///
    /// Defaults to `debug`.
    #[arg(long, value_name = "NAME", default_value = "debug")]
    pub profile: String,

    /// Override the profile's debug comment level in generated Lua.
    #[arg(long, value_name = "LEVEL")]
    pub debug_level: Option<u8>,

    /// Only run tests whose name contains this filter (like `cargo test FILTER`).
    #[arg(value_name = "FILTER")]
    pub filter: Option<String>,

    /// Skip `cargo check --tests` before transpile (not recommended).
    #[arg(long)]
    pub skip_typecheck: bool,

    /// Open a Factorio window instead of running headless, so you can watch the suite.
    ///
    /// After the suite finishes, Factorio stays open until you close it.
    #[arg(long)]
    pub gui: bool,

    /// Kill Factorio if the suite does not finish within this many seconds.
    #[arg(long, value_name = "SECS", default_value_t = 120)]
    pub timeout: u64,

    /// Keep Factorio alive after the suite (for Bacon hot-reload re-runs).
    #[arg(long)]
    pub listen: bool,

    /// Rebuild, sync into the listen test-run, wait for the next suite report.
    ///
    /// Starts a listen Factorio process if none is running. Intended for Bacon.
    #[arg(long)]
    pub rerun: bool,
}

#[derive(Debug, Parser)]
pub struct BenchArgs {
    /// Path to the project directory or `Factorio.toml` file.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Transpile profile from `Factorio.toml`.
    ///
    /// Defaults to `release`.
    #[arg(long, value_name = "NAME", default_value = "release")]
    pub profile: String,

    /// Override the profile's debug comment level in generated Lua.
    #[arg(long, value_name = "LEVEL")]
    pub debug_level: Option<u8>,

    /// Only run benches whose name contains this filter substring.
    #[arg(value_name = "FILTER")]
    pub filter: Option<String>,

    /// Skip `cargo check --tests` before transpile (not recommended).
    #[arg(long)]
    pub skip_typecheck: bool,

    /// Open a Factorio window instead of running headless.
    #[arg(long)]
    pub gui: bool,

    /// Kill Factorio if the bench suite does not finish within this many seconds.
    #[arg(long, value_name = "SECS", default_value_t = 120)]
    pub timeout: u64,
}
