pub(crate) mod artifact;
pub(crate) mod audit;
pub(crate) mod domain;
pub(crate) mod graph;
pub(crate) mod member;
pub(crate) mod phase;
pub(crate) mod task;

use crate::skill::Args as Skill;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

pub use artifact::Args as Artifact;
pub use audit::Args as Audit;
pub use domain::Args as Domain;
pub use graph::Args as Graph;
pub use member::Args as Member;
pub use phase::Args as Phase;
pub use task::Args as Task;

#[derive(Args)]
pub struct Observe {
    #[arg(long)]
    pub observe: bool,
    #[arg(long = "github-command", requires = "observe")]
    pub command: Option<PathBuf>,
    #[arg(
        long = "observe-timeout",
        default_value_t = 10,
        value_parser = clap::value_parser!(u64).range(1..=60),
        requires = "observe"
    )]
    pub timeout: u64,
}

#[derive(Parser)]
#[command(version = plumb::version!("CONCORD"), about)]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,
    #[arg(long, global = true)]
    pub home: Option<PathBuf>,
    #[arg(long, global = true)]
    pub releases: Option<String>,
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Inspect Concord configuration")]
    Config(Config),
    #[command(about = "Manage estate Domains")]
    Domain(Domain),
    #[command(about = "Manage Task lifecycle, facts, and dependencies")]
    Task(Task),
    #[command(about = "Settle and read frozen Task Phases")]
    Phase(Phase),
    #[command(about = "Manage repository worktree Members")]
    Member(Member),
    #[command(about = "Manage direct private Artifact payload")]
    Artifact(Artifact),
    #[command(about = "Inspect the Task dependency graph")]
    Graph(Graph),
    #[command(about = "Audit estate, graph, and external agreement")]
    Audit(Audit),
    #[command(about = "Manage Concord agent skill installations")]
    Skill(Skill),
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Config(args) => args.command.name(),
            Self::Domain(args) => args.command.name(),
            Self::Task(args) => args.command.name(),
            Self::Phase(args) => args.command.name(),
            Self::Member(args) => args.command.name(),
            Self::Artifact(args) => args.command.name(),
            Self::Graph(args) => args.command.name(),
            Self::Audit(_) => "audit",
            Self::Skill(args) => args.command.name(),
        }
    }

    pub(crate) fn activity(&self) -> Option<Vec<&str>> {
        match self {
            Self::Task(args) => args.command.activity(),
            Self::Phase(args) => args.command.activity(),
            Self::Member(args) => args.command.activity(),
            Self::Artifact(args) => Some(args.command.activity()),
            Self::Graph(args) => args.command.activity(),
            Self::Audit(args) => args.task.as_deref().map(|task| vec![task]),
            Self::Config(_) | Self::Domain(_) | Self::Skill(_) => None,
        }
    }
}

#[derive(Args)]
pub struct Config {
    #[command(subcommand)]
    pub command: Configure,
}

#[derive(Subcommand)]
pub enum Configure {
    #[command(about = "Print the selected config path")]
    Path,
    #[command(about = "Print resolved runtime configuration")]
    Show,
}

impl Configure {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Path => "config.path",
            Self::Show => "config.show",
        }
    }
}
