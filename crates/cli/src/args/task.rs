use clap::{Args, Subcommand};

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
    #[command(about = "Manage outgoing future-task todos")]
    Todo {
        #[command(subcommand)]
        command: TaskTodoCommand,
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

#[derive(Subcommand)]
pub enum TaskTodoCommand {
    #[command(about = "Link an existing task or create and link a repo-less task")]
    Add {
        task: String,
        target: String,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Remove one outgoing task todo")]
    Remove {
        task: String,
        target: String,
        #[arg(long)]
        apply: bool,
    },
}
