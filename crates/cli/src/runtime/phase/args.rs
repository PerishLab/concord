use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Command {
    #[command(
        about = "Prepare one Issue Phase set or clear; input: {reason, value: Clarify|Deliver|Verify|Settle|null}. Requires explicit [phase] native identity binding; no inferred transitions"
    )]
    Prepare {
        issue: String,
        #[arg(long, default_value = "-")]
        input: PathBuf,
        #[command(flatten)]
        remote: Remote,
    },
    #[command(
        about = "Apply one fresh prepared Phase change, then independently read back; no remote CAS or automatic replay"
    )]
    Apply {
        #[arg(long, default_value = "-")]
        input: PathBuf,
        #[arg(long)]
        apply: bool,
        #[command(flatten)]
        remote: Remote,
    },
    #[command(
        about = "Read current native facts for a prepared Phase change without replay or authorship claims"
    )]
    Recover {
        #[arg(long, default_value = "-")]
        input: PathBuf,
        #[command(flatten)]
        remote: Remote,
    },
}

#[derive(Args)]
pub struct Remote {
    #[arg(long = "github-command", value_name = "PROGRAM", help = crate::args::PROGRAM)]
    pub command: PathBuf,
    #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
    pub timeout: u64,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Prepare { .. } => "issue.phase.prepare",
            Self::Apply { .. } => "issue.phase.apply",
            Self::Recover { .. } => "issue.phase.recover",
        }
    }
}
