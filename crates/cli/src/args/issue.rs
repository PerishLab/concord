use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Explicit acceptance-bound Issue closure without opening local execution", long_about = super::close::HELP)]
    Close {
        #[command(subcommand)]
        command: super::close::Command,
    },
    #[command(
        about = "Explicit native Issue Phase operations; requires [phase] owner/organization/field and [phase.options] exact native IDs; no stored values or history"
    )]
    Phase {
        #[command(subcommand)]
        command: crate::phase::Command,
    },
    #[command(about = "Bootstrap a fresh Issue-execution estate")]
    Bootstrap,
    #[command(about = "Project one bounded GitHub Issue brief without persisting it")]
    Brief {
        issue: String,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
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
    #[command(about = "Preflight proposed or existing Issue work")]
    Preflight {
        #[command(subcommand)]
        command: Preflight,
    },
    #[command(about = "Derive one bounded graph from native GitHub relationships")]
    Graph {
        issue: String,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
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
    Ready(Relations),
    #[command(about = "Post a Markdown Issue comment with appended execution card and marker")]
    Comment {
        issue: String,
        #[arg(
            long,
            value_name = "PATH|-",
            default_value = "-",
            help = "Markdown comment body (stdin by default); execution card and marker are appended. Bare JSON documents are refused; fenced JSON in prose is allowed"
        )]
        input: PathBuf,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Prepare and land exact Issue-led pull requests", long_about = DELIVERY)]
    Delivery {
        #[command(subcommand)]
        command: Delivery,
    },
    #[command(about = "Attach local execution to one readable typed GitHub Issue")]
    Attach {
        issue: String,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
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
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Revalidate an exact Issue delivery preparation")]
    Validate {
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        input: PathBuf,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
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
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
}

#[derive(Subcommand)]
pub enum Preflight {
    #[command(about = "Find probable duplicate work for a proposed Issue")]
    Proposed {
        repository: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        outcome: String,
        #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(long = "page-size", default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=100))]
        page_size: u16,
        #[arg(long = "max-pages", default_value_t = 4, value_parser = clap::value_parser!(u16).range(1..=20))]
        max_pages: u16,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Explain native relationships and active execution for an existing Issue")]
    Existing(Relations),
}

#[derive(clap::Args)]
pub struct Relations {
    pub issue: String,
    #[arg(long = "github-command", value_name = "PROGRAM", help = super::PROGRAM)]
    pub command: PathBuf,
    #[arg(long = "page-size", default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=100))]
    pub page_size: u16,
    #[arg(long = "max-pages", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100))]
    pub max_pages: u16,
    #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
    pub timeout: u64,
}

#[derive(Subcommand)]
pub enum Delivery {
    #[command(about = "Prepare one exact Issue-led pull-request plan without provider mutation")]
    Prepare {
        issue: String,
        #[arg(
            long,
            default_value = "plumb",
            help = "Explicit proof authority: plumb or wharf-native"
        )]
        authority: concord_core::issue_delivery::Mode,
        #[arg(long)]
        revision: i64,
        #[arg(long, default_value = "main")]
        base: String,
        #[arg(
            long,
            help = "Carry preparation failures through a streamed delivery handoff"
        )]
        handoff: bool,
        #[arg(long = "github-command", default_value = "gh", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(
            long,
            default_value = "github",
            help = "Delivery adaptor; only github on github.com is supported; command override does not select a platform"
        )]
        forge: String,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    #[command(about = "Revalidate and merge one exact Issue-led pull-request plan")]
    Land {
        issue: String,
        #[arg(
            long = "no-wait",
            action = clap::ArgAction::SetFalse,
            default_value_t = true,
            help = "Return pending on stdout with exit 3 without waiting for CI; rerun the same plan. Default waits up to 30 minutes; cross-run budgets belong to the caller"
        )]
        wait: bool,
        #[arg(
            long,
            default_value = "plumb",
            help = "Must match the prepared proof authority"
        )]
        authority: concord_core::issue_delivery::Mode,
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        plan: PathBuf,
        #[arg(long = "github-command", default_value = "gh", value_name = "PROGRAM", help = super::PROGRAM)]
        command: PathBuf,
        #[arg(
            long,
            default_value = "github",
            help = "Delivery adaptor; only github on github.com is supported; command override does not select a platform"
        )]
        forge: String,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Bootstrap => "issue.bootstrap",
            Self::Close { command } => command.name(),
            Self::Phase { command } => command.name(),
            Self::Brief { .. } => "issue.brief",
            Self::Preflight { command } => command.name(),
            Self::Graph { .. } => "issue.graph",
            Self::Ready(_) => "issue.ready",
            Self::Comment { .. } => "issue.comment",
            Self::Delivery { command } => command.name(),
            Self::Attach { .. } => "issue.attach",
            Self::Show { .. } => "issue.show",
            Self::Prepare { .. } => "issue.prepare",
            Self::Validate { .. } => "issue.validate",
            Self::Reconcile { .. } => "issue.reconcile",
        }
    }
}

impl Preflight {
    fn name(&self) -> &'static str {
        match self {
            Self::Proposed { .. } => "issue.preflight.proposed",
            Self::Existing(_) => "issue.preflight.existing",
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

const DELIVERY: &str = "Prepare and land exact Issue-led pull requests through the qualified GitHub adaptor. Delivery defaults to the external gh program on PATH; --github-command overrides one executable, never the platform or host. Only --forge github and github.com origins are supported. API and pull commands explicitly bind github.com, overriding gh host defaults. No credentials are held by Concord.

New handoffs use envelope v2 with forge kind, host, repository coordinate and repository node. Legacy v1 handoffs retain their GitHub-only meaning and undergo the same current origin, proof, acceptance and provider checks; they cannot opt into another forge. Other or conflicting identities refuse before provider mutation. Existing estate identities are unchanged.

The adaptor exposes bounded Issue/acceptance observations, exact pull facts, checks, mutation and independent readback. Missing platform capabilities refuse rather than appear empty or successful. Codehull, Forgejo and Gitee are unsupported; each requires its own identity namespace and qualification of Issue types/relations, checks, merge and readback. Project capability is separate; URL substitution or a different command executable does not supply a new adaptor. Unknown write outcomes require observation before resuming, never blind replay.";
