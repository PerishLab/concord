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
    #[command(
        about = "Apply a versioned JSON change-set from stdin by default",
        after_help = concord_core::Patch::SHAPE
    )]
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
    #[command(about = "Manage the declared issue or work-item coordinate")]
    Reference {
        #[command(subcommand)]
        command: Reference,
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

#[derive(Subcommand)]
pub enum Reference {
    #[command(about = "Declare or replace the Task forge coordinate")]
    Set {
        task: String,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        number: i64,
        #[arg(long)]
        revision: i64,
    },
    #[command(about = "Remove the Task forge coordinate")]
    Remove {
        task: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "task.list",
            Self::Brief { .. } => "task.brief",
            Self::Show { .. } => "task.show",
            Self::Start { .. } => "task.start",
            Self::Change { .. } => "task.change",
            Self::Rename { .. } => "task.rename",
            Self::Rehome { .. } => "task.rehome",
            Self::Dependency { command } => command.name(),
            Self::Reference { command } => command.name(),
            Self::Finish { .. } => "task.finish",
        }
    }

    pub(crate) fn activity(&self) -> Option<Vec<&str>> {
        match self {
            Self::Show { task } => Some(vec![task]),
            Self::Rename { task, .. } => Some(vec![task]),
            Self::Rehome { task, .. } => Some(vec![task]),
            Self::Dependency { command } => Some(command.activity()),
            Self::Reference { command } => Some(command.activity()),
            Self::Finish { task, .. } => Some(vec![task]),
            Self::List { .. } | Self::Brief { .. } | Self::Start { .. } | Self::Change { .. } => {
                None
            }
        }
    }
}

impl Reference {
    fn name(&self) -> &'static str {
        match self {
            Self::Set { .. } => "task.reference.set",
            Self::Remove { .. } => "task.reference.remove",
        }
    }

    fn activity(&self) -> Vec<&str> {
        match self {
            Self::Set { task, .. } | Self::Remove { task, .. } => vec![task],
        }
    }
}

impl Dependency {
    fn name(&self) -> &'static str {
        match self {
            Self::Add { .. } => "task.dependency.add",
            Self::Set { .. } => "task.dependency.set",
            Self::Remove { .. } => "task.dependency.remove",
            Self::List { .. } => "task.dependency.list",
        }
    }

    fn activity(&self) -> Vec<&str> {
        match self {
            Self::Add { source, target, .. } => vec![source, target],
            Self::Set { source, target, .. } => vec![source, target],
            Self::Remove { source, target, .. } => vec![source, target],
            Self::List { task, .. } => vec![task],
        }
    }
}
