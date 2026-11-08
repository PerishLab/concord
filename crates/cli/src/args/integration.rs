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
        about = "Prepare or apply an explicit idle Integration relocation",
        long_about = "Rebind only an explicitly relocated idle checkout. Move the clean main checkout while all Members are settled, then prepare against its exact registered repository/node/key and new path. The original path must be absent. The target must belong to that same GitHub node, be clean on main equal to its fetched origin/main, and have no auxiliary worktrees. Preparation binds Space identity, the complete original registration and canonical target path/common/HEAD. Apply without --apply only preflights; --apply repeats current checks under locks and updates path/common in one batch. Changed facts and unrelated audit faults refuse. Concord never moves files, repairs Git metadata, fetches remote refs, or replaces Space identity, Integration keys, historical Anchors or Artifacts. Old plans must be prepared again."
    )]
    Rebind {
        #[command(subcommand)]
        command: Rebinding,
    },
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
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10)]
        timeout: u64,
    },
    #[command(about = "Reconcile a renamed repository Integration")]
    Reconcile {
        repository: String,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10)]
        timeout: u64,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Rebind { command } => command.name(),
            Self::List => "integration.list",
            Self::Retire { .. } => "integration.retire",
            Self::Register { .. } => "integration.register",
            Self::Reconcile { .. } => "integration.reconcile",
        }
    }
}

#[derive(Subcommand)]
pub enum Rebinding {
    #[command(about = "Bind a plan to an absent original checkout and healthy relocated main")]
    Prepare {
        repository: String,
        #[arg(long)]
        node: String,
        #[arg(long)]
        key: i64,
        #[arg(long)]
        path: PathBuf,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10)]
        timeout: u64,
    },
    #[command(about = "Recheck an exact relocation plan; --apply changes only its registration")]
    Apply {
        repository: String,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10)]
        timeout: u64,
    },
}

impl Rebinding {
    fn name(&self) -> &'static str {
        match self {
            Self::Prepare { .. } => "integration.rebind.prepare",
            Self::Apply { .. } => "integration.rebind.apply",
        }
    }
}
