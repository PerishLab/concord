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
}
