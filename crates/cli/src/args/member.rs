use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "List estate-managed worktree Members")]
    List {
        #[arg(long)]
        task: Option<String>,
    },
    #[command(about = "Inspect one Member's current local Git health")]
    Status { task: String, member: String },
    #[command(about = "Create and attach a Git worktree Member")]
    Attach {
        task: String,
        name: String,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long, required = true, num_args = 1..)]
        claim: Vec<String>,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Expand one Member claim")]
    Claim {
        task: String,
        member: String,
        #[arg(long, required = true, num_args = 1..)]
        claim: Vec<String>,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Shrink one Member claim to an explicit set")]
    Narrow {
        task: String,
        member: String,
        #[arg(long, required = true, num_args = 1..)]
        claim: Vec<String>,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Create a current Plumb Boundary proof")]
    Prove {
        task: String,
        member: String,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Remove one clean and landed Member")]
    Release {
        task: String,
        member: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Remove one clean unlanded Member against matching Artifacts")]
    Retire {
        task: String,
        member: String,
        #[arg(long, required = true, num_args = 1..)]
        artifacts: Vec<String>,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
}

impl Command {
    pub(crate) fn activity(&self) -> Option<(&'static str, Vec<&str>)> {
        match self {
            Self::List { task } => task.as_deref().map(|task| ("member.list", vec![task])),
            Self::Status { task, .. } => Some(("member.status", vec![task])),
            Self::Attach { task, .. } => Some(("member.attach", vec![task])),
            Self::Claim { task, .. } => Some(("member.claim", vec![task])),
            Self::Narrow { task, .. } => Some(("member.narrow", vec![task])),
            Self::Prove { task, .. } => Some(("member.prove", vec![task])),
            Self::Release { task, .. } => Some(("member.release", vec![task])),
            Self::Retire { task, .. } => Some(("member.retire", vec![task])),
        }
    }
}
