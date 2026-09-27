use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Inspect an exact v0.12.10 source without changing it")]
    Preflight,
    #[command(about = "Prepare and verify a private v0.13.0 estate stage")]
    Prepare { fingerprint: String },
    #[command(about = "Atomically activate one exact prepared migration")]
    Apply {
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        plan: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Restore the exact v0.12.10 backup from one receipt")]
    Rollback {
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        receipt: PathBuf,
        #[arg(long)]
        apply: bool,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Preflight => "migration.preflight",
            Self::Prepare { .. } => "migration.prepare",
            Self::Apply { .. } => "migration.apply",
            Self::Rollback { .. } => "migration.rollback",
        }
    }
}
