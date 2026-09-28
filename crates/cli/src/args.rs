pub(crate) mod artifact;
pub(crate) mod audit;
pub(crate) mod issue;
pub(crate) mod member;

use crate::skill::Args as Skill;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

pub use artifact::Args as Artifact;
pub use audit::Args as Audit;
pub use issue::Args as Issue;
pub use member::Args as Member;

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
    #[command(about = "Read code-addressed recovery guidance")]
    Cookbook(Cookbook),
    #[command(about = "Anchor local execution to typed GitHub Issues")]
    Issue(Issue),
    #[command(about = "Manage repository worktree Members")]
    Member(Member),
    #[command(about = "Manage direct private Artifact payload")]
    Artifact(Artifact),
    #[command(about = "Audit Issue estate and external agreement")]
    Audit(Audit),
    #[command(about = "Manage Concord agent skill installations")]
    Skill(Skill),
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Config(args) => args.command.name(),
            Self::Cookbook(_) => "cookbook",
            Self::Issue(args) => args.command.name(),
            Self::Member(args) => args.command.name(),
            Self::Artifact(args) => args.command.name(),
            Self::Audit(_) => "audit",
            Self::Skill(args) => args.command.name(),
        }
    }

    pub(crate) fn activity(&self) -> Option<Vec<&str>> {
        match self {
            Self::Issue(_) => None,
            Self::Member(args) => args.command.activity(),
            Self::Artifact(args) => Some(args.command.activity()),
            Self::Audit(_) | Self::Config(_) | Self::Cookbook(_) | Self::Skill(_) => None,
        }
    }
}

#[derive(Args)]
pub struct Cookbook {
    pub code: Option<String>,
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
