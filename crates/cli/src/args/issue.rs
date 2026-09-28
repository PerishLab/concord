use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Bootstrap a fresh Issue-execution estate")]
    Bootstrap,
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
    #[command(about = "Preflight a proposed Issue against GitHub and active execution")]
    Preflight {
        repository: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        outcome: String,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "page-size", default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=100))]
        page_size: u16,
        #[arg(long = "max-pages", default_value_t = 4, value_parser = clap::value_parser!(u16).range(1..=20))]
        max_pages: u16,
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
    #[command(about = "Prepare and land exact Issue-led pull requests")]
    Delivery {
        #[command(subcommand)]
        command: Delivery,
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

#[derive(Subcommand)]
pub enum Delivery {
    #[command(about = "Prepare one exact Issue-led pull-request plan without provider mutation")]
    Prepare {
        issue: String,
        member: String,
        #[arg(long)]
        revision: i64,
        #[arg(long, default_value = "main")]
        base: String,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Revalidate and merge one exact Issue-led pull-request plan")]
    Land {
        issue: String,
        member: String,
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        plan: PathBuf,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Bootstrap => "issue.bootstrap",
            Self::Brief { .. } => "issue.brief",
            Self::Preflight { .. } => "issue.preflight",
            Self::Graph { .. } => "issue.graph",
            Self::Ready { .. } => "issue.ready",
            Self::Delivery { command } => command.name(),
            Self::Attach { .. } => "issue.attach",
            Self::Show { .. } => "issue.show",
            Self::Prepare { .. } => "issue.prepare",
            Self::Validate { .. } => "issue.validate",
            Self::Reconcile { .. } => "issue.reconcile",
        }
    }
}

impl Delivery {
    fn name(&self) -> &'static str {
        match self {
            Self::Prepare { .. } => "issue.delivery.prepare",
            Self::Land { .. } => "issue.delivery.land",
        }
    }
}
