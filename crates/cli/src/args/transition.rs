use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Inventory an exact v0.13.0 source without changing it")]
    Inventory,
    #[command(about = "Validate one complete v0.13.0 disposition plan without changing it")]
    Preflight {
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        plan: PathBuf,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Inventory => "transition.inventory",
            Self::Preflight { .. } => "transition.preflight",
        }
    }
}
