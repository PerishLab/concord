use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Project one bounded GitHub Issue brief without persisting it")]
    Brief {
        issue: String,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "page-size", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100))]
        page_size: u16,
        #[arg(long = "sub-issues-after")]
        sub_issues_after: Option<String>,
        #[arg(long = "blocked-by-after")]
        blocked_by_after: Option<String>,
        #[arg(long = "blocking-after")]
        blocking_after: Option<String>,
        #[arg(long = "pulls-after")]
        pulls_after: Option<String>,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Derive one bounded graph from native GitHub relationships")]
    Graph {
        issue: String,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "page-size", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100))]
        page_size: u16,
        #[arg(long = "max-nodes", default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=500))]
        max_nodes: u16,
        #[arg(long = "max-pages", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100))]
        max_pages: u16,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Evaluate Issue closure readiness from current GitHub facts")]
    Ready {
        issue: String,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "page-size", default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=100))]
        page_size: u16,
        #[arg(long = "max-pages", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100))]
        max_pages: u16,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
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
            Self::Brief { .. } => "issue.brief",
            Self::Graph { .. } => "issue.graph",
            Self::Ready { .. } => "issue.ready",
            Self::Attach { .. } => "issue.attach",
            Self::Show { .. } => "issue.show",
            Self::Prepare { .. } => "issue.prepare",
            Self::Validate { .. } => "issue.validate",
            Self::Reconcile { .. } => "issue.reconcile",
        }
    }
}
