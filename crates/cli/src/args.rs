use crate::skill::SkillArgs;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about)]
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
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Subcommand)]
pub enum TaskCommand {
    #[command(about = "List registered tasks")]
    List {
        #[arg(long)]
        domain: Option<String>,
    },
    #[command(about = "Show one resolved task")]
    Show { task: String },
    #[command(about = "Start a repo-less task")]
    Start {
        task: String,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Rename a task through an explicit migration")]
    Rename {
        task: String,
        name: String,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Move a task to another home domain")]
    Rehome {
        task: String,
        domain: String,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "Remove an empty task and registry entry")]
    Finish {
        task: String,
        #[arg(long)]
        apply: bool,
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
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Prove cleanliness and landed reachability")]
    Preflight { task: String },
    #[command(about = "Remove a clean, reachable landed worktree")]
    RemoveLanded {
        task: String,
        name: String,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Subcommand)]
pub enum MemoryCommand {
    #[command(about = "Create .task/ and its initial MAIN.md")]
    Init {
        task: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Read MAIN.md with its content revision")]
    Read { task: String },
    #[command(about = "Replace MAIN.md at an expected revision")]
    Write {
        task: String,
        #[arg(long)]
        expect: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Allocate a phase and replace MAIN.md")]
    Settle {
        task: String,
        #[arg(long)]
        expect: String,
        #[arg(long)]
        phase_file: PathBuf,
        #[arg(long)]
        main_file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Remove the exact .task/ tree")]
    Remove {
        task: String,
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
