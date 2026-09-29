use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "List registered repository Integrations")]
    List,
    #[command(about = "Register one clean synchronized main Integration")]
    Register {
        repository: String,
        #[arg(long)]
        path: PathBuf,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10)]
        timeout: u64,
    },
    #[command(about = "Reconcile a renamed repository Integration")]
    Reconcile {
        repository: String,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10)]
        timeout: u64,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::List => "integration.list",
            Self::Register { .. } => "integration.register",
            Self::Reconcile { .. } => "integration.reconcile",
        }
    }
}
