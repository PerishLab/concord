use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Attach local execution to one readable typed GitHub Issue")]
    Attach {
        issue: String,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Read one Issue execution anchor")]
    Show { issue: String },
    #[command(about = "Bind an exact Issue observation for delivery preparation")]
    Prepare {
        issue: String,
        #[arg(long)]
        revision: i64,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Revalidate an exact Issue delivery preparation")]
    Validate {
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        input: PathBuf,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Reconcile a moved Issue when its stable node is unchanged")]
    Reconcile {
        issue: String,
        coordinate: String,
        #[arg(long)]
        revision: i64,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Attach { .. } => "issue.attach",
            Self::Show { .. } => "issue.show",
            Self::Prepare { .. } => "issue.prepare",
            Self::Validate { .. } => "issue.validate",
            Self::Reconcile { .. } => "issue.reconcile",
        }
    }
}
