pub(crate) mod artifact;
pub(crate) mod audit;
pub(crate) mod domain;
pub(crate) mod graph;
pub(crate) mod member;
pub(crate) mod migration;
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
pub use migration::Args as Migration;
pub use phase::Args as Phase;
pub use task::Args as Task;

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
    #[command(about = "Stage and activate the v0.10.0 estate migration")]
    Migration(Migration),
    #[command(about = "Manage Concord agent skill installations")]
    Skill(Skill),
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Config(_) => "config",
            Self::Domain(_) => "domain",
            Self::Task(_) => "task",
            Self::Phase(_) => "phase",
            Self::Member(_) => "member",
            Self::Artifact(_) => "artifact",
            Self::Graph(_) => "graph",
            Self::Audit(_) => "audit",
            Self::Migration(_) => "migration",
            Self::Skill(_) => "skill",
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
