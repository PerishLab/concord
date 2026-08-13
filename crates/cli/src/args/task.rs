use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "List Tasks from the estate")]
    List {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        retired: bool,
    },
    #[command(about = "Project bounded current Task facts for one Domain")]
    Brief {
        #[arg(long)]
        domain: String,
        #[arg(long)]
        after: Option<String>,
    },
    #[command(about = "Read one Task and its current structured facts")]
    Show { task: String },
    #[command(about = "Start an estate-only Task in an existing Domain")]
    Start { domain: String, name: String },
    #[command(about = "Apply a versioned JSON change-set from stdin by default")]
    Change {
        #[arg(long, value_name = "PATH|-", default_value = "-")]
        input: PathBuf,
    },
    #[command(about = "Rename a Task while retaining its permanent identity")]
    Rename {
        task: String,
        name: String,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Move a Task to another Domain")]
    Rehome {
        task: String,
        domain: String,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Manage directed Task dependencies")]
    Dependency {
        #[command(subcommand)]
        command: Dependency,
    },
    #[command(about = "Retire a Task and cut every incident dependency")]
    Finish {
        task: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        graph: i64,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand)]
pub enum Dependency {
    #[command(about = "Declare SOURCE depends_on TARGET")]
    Add {
        source: String,
        target: String,
        #[arg(long)]
        weight: String,
        #[arg(long)]
        revision: i64,
        #[arg(long = "create-target")]
        create: bool,
    },
    #[command(about = "Set the weight of one direct dependency")]
    Set {
        source: String,
        target: String,
        #[arg(long)]
        weight: String,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Remove one direct dependency with a retained reason")]
    Remove {
        source: String,
        target: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        apply: bool,
    },
    #[command(about = "List direct dependencies touching one Task")]
    List {
        task: String,
        #[arg(long, default_value = "both")]
        direction: String,
    },
}
