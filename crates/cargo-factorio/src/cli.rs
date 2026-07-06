#[derive(Debug, Clone, Copy, clap::Subcommand)]
pub enum Command {
    Init,
    Build,
}

#[derive(Debug, Clone, Copy, clap::Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,
}
