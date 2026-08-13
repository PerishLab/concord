use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "List immutable Phases for one Task")]
    List { task: String },
    #[command(about = "Settle a versioned JSON envelope from stdin by default")]
    Settle {
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        input: PathBuf,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "phase.list",
            Self::Settle { .. } => "phase.settle",
        }
    }

    pub(crate) fn activity(&self) -> Option<Vec<&str>> {
        match self {
            Self::List { task } => Some(vec![task]),
            Self::Settle { .. } => None,
        }
    }
}
