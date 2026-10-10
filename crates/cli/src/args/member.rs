use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "List Issue-anchored worktree Members")]
    List {
        #[arg(long)]
        issue: Option<String>,
    },
    #[command(about = "Inspect one Issue Member's local Git health")]
    Status { issue: String },
    #[command(about = "Start one Issue Member from synchronized main")]
    Start {
        issue: String,
        #[arg(long, required = true, num_args = 1..)]
        claim: Vec<String>,
        #[arg(long)]
        revision: i64,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Expand one Issue Member claim")]
    Claim {
        issue: String,
        #[arg(long, required = true, num_args = 1..)]
        claim: Vec<String>,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Shrink one Issue Member claim to an explicit set")]
    Narrow {
        issue: String,
        #[arg(long, required = true, num_args = 1..)]
        claim: Vec<String>,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Create a current Plumb Boundary proof")]
    Prove {
        issue: String,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Prepare or revalidate one exact Issue Member landing")]
    Landing {
        #[command(subcommand)]
        command: Landing,
    },
    #[command(about = "Manage one Issue Member pull coordinate")]
    Reference {
        #[command(subcommand)]
        command: Reference,
    },
    #[command(about = "Remove one clean and landed Issue Member")]
    Release {
        issue: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Restore one declared missing Member at its exact retained head")]
    Recover {
        issue: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        head: String,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Complete one unchanged Issue Member without delivery")]
    Complete {
        issue: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Remove one clean unlanded Issue Member against matching Artifacts")]
    Retire {
        issue: String,
        #[arg(long, required = true, num_args = 1..)]
        artifacts: Vec<String>,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand)]
pub enum Landing {
    #[command(about = "Prepare an exact Issue Member landing plan")]
    Prepare {
        issue: String,
        #[arg(long, default_value = "main")]
        base: String,
        #[arg(long, default_value = "")]
        title: String,
        #[arg(long, default_value = "")]
        body: String,
        #[arg(long)]
        guard_schema: String,
        #[arg(long)]
        guard_tree: String,
        #[arg(long)]
        guard_digest: String,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Revalidate an exact Issue Member landing plan")]
    Ready {
        issue: String,
        #[arg(long, default_value = "-")]
        plan: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum Reference {
    #[command(about = "Declare an Issue Member pull coordinate")]
    Set {
        issue: String,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        number: i64,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Remove an Issue Member pull coordinate")]
    Remove {
        issue: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        number: i64,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "member.list",
            Self::Status { .. } => "member.status",
            Self::Start { .. } => "member.start",
            Self::Claim { .. } => "member.claim",
            Self::Narrow { .. } => "member.narrow",
            Self::Prove { .. } => "member.prove",
            Self::Landing { command } => command.name(),
            Self::Reference { command } => command.name(),
            Self::Release { .. } => "member.release",
            Self::Recover { .. } => "member.recover",
            Self::Complete { .. } => "member.complete",
            Self::Retire { .. } => "member.retire",
        }
    }

    pub(crate) fn activity(&self) -> Option<Vec<&str>> {
        match self {
            Self::List { issue } => issue.as_deref().map(|held| vec![held]),
            Self::Status { issue }
            | Self::Start { issue, .. }
            | Self::Claim { issue, .. }
            | Self::Narrow { issue, .. }
            | Self::Prove { issue, .. }
            | Self::Release { issue, .. }
            | Self::Recover { issue, .. }
            | Self::Complete { issue, .. }
            | Self::Retire { issue, .. } => Some(vec![issue]),
            Self::Landing { command } => Some(vec![command.issue()]),
            Self::Reference { command } => Some(vec![command.issue()]),
        }
    }
}

impl Landing {
    fn name(&self) -> &'static str {
        match self {
            Self::Prepare { .. } => "member.landing.prepare",
            Self::Ready { .. } => "member.landing.ready",
        }
    }
    pub(crate) fn issue(&self) -> &str {
        match self {
            Self::Prepare { issue, .. } | Self::Ready { issue, .. } => issue,
        }
    }
}

impl Reference {
    fn name(&self) -> &'static str {
        match self {
            Self::Set { .. } => "member.reference.set",
            Self::Remove { .. } => "member.reference.remove",
        }
    }
    pub(crate) fn issue(&self) -> &str {
        match self {
            Self::Set { issue, .. } | Self::Remove { issue, .. } => issue,
        }
    }
}
