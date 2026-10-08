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
    #[command(
        about = "Preflight or explicitly retire one idle Integration without deleting payload",
        long_about = "Explicitly retire an idle repository after the operator removes it from the managed domain. Read integration list and bind its exact repository, node and key. Without --apply this only preflights; --apply repeats locked current-state checks and ends only that registration. Live Members, stale identity and unrelated agreement faults refuse. An absent target checkout does not block this recovery, but never triggers it automatically. No provider observation is required or interpreted as authorization. Git payload, historical Issue anchors and private Artifacts are retained; no local checkout or remote repository is deleted."
    )]
    Retire {
        repository: String,
        #[arg(long)]
        node: String,
        #[arg(long)]
        key: i64,
        #[arg(long)]
        apply: bool,
    },
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
            Self::Retire { .. } => "integration.retire",
            Self::Register { .. } => "integration.register",
            Self::Reconcile { .. } => "integration.reconcile",
        }
    }
}
