mod memory;
mod task;

use crate::skill::SkillArgs;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

pub use memory::{MemoryArgs, MemoryCommand, MemoryPhaseCommand};
pub use task::{TaskArgs, TaskCommand, TaskTodoCommand};

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
    Config(ConfigArgs),
    #[command(about = "List or initialize managed domains")]
    Domain(DomainArgs),
    #[command(about = "Manage repository annotations")]
    Repo(RepoArgs),
    #[command(about = "Manage task lifecycle")]
    Task(TaskArgs),
    #[command(about = "Manage repository member seats")]
    Member(MemberArgs),
    #[command(about = "Read and replace task memory")]
    Memory(MemoryArgs),
    #[command(about = "Manage opaque resource seats")]
    Resource(ResourceArgs),
    #[command(about = "Normalize private task permissions")]
    Permissions(PermissionArgs),
    #[command(about = "Manage Concord agent skill installations")]
    Skill(SkillArgs),
    #[command(about = "Audit protocol agreement")]
    Audit(AuditArgs),
}

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    #[command(about = "Print the selected config path")]
    Path,
    #[command(about = "Print resolved runtime configuration")]
    Show,
}

#[derive(Args)]
pub struct DomainArgs {
    #[command(subcommand)]
    pub command: DomainCommand,
}

#[derive(Subcommand)]
pub enum DomainCommand {
    #[command(about = "List managed domains")]
    List,
    #[command(about = "Initialize a domain registry")]
    Init {
        name: String,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Migrate a registry to the current protocol version")]
    Migrate {
        domain: String,
        #[arg(long)]
        claim: Vec<String>,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Args)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub command: RepoCommand,
}

#[derive(Subcommand)]
pub enum RepoCommand {
    #[command(about = "Add a repository annotation")]
    Annotate {
        domain: String,
        name: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Args)]
pub struct MemberArgs {
    #[command(subcommand)]
    pub command: MemberCommand,
}

#[derive(Subcommand)]
pub enum MemberCommand {
    #[command(about = "Create and register a Git worktree seat")]
    Add {
        task: String,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long)]
        orphan: bool,
        #[arg(long, required = true, num_args = 1..)]
        write: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Prove cleanliness and landed reachability")]
    Preflight { task: String },
    #[command(about = "Expand a member write claim")]
    Claim {
        task: String,
        name: String,
        #[arg(long, required = true, num_args = 1..)]
        write: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Prove committed changes stay inside a member write claim")]
    Boundary { task: String, name: String },
    #[command(about = "Remove a clean, reachable landed worktree")]
    RemoveLanded {
        task: String,
        name: String,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Args)]
pub struct ResourceArgs {
    #[command(subcommand)]
    pub command: ResourceCommand,
}

#[derive(Subcommand)]
pub enum ResourceCommand {
    #[command(about = "List allocated resource seats")]
    List { task: String },
    #[command(about = "Resolve one resource seat")]
    Show { task: String, name: String },
    #[command(about = "Allocate an empty private resource seat")]
    Allocate {
        task: String,
        name: String,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Import a file or tree into a private seat")]
    Import {
        task: String,
        name: String,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Remove one exact resource seat")]
    Remove {
        task: String,
        name: String,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Args)]
pub struct PermissionArgs {
    #[command(subcommand)]
    pub command: PermissionCommand,
}

#[derive(Subcommand)]
pub enum PermissionCommand {
    #[command(about = "Restore private modes without changing payload")]
    Normalize {
        task: String,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Args)]
pub struct AuditArgs {
    pub task: Option<String>,
    #[arg(long)]
    pub domain: Option<String>,
    #[arg(long)]
    pub space: bool,
}
