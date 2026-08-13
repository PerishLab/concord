use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "List direct filesystem Artifacts")]
    List { task: String },
    #[command(about = "Resolve one direct filesystem Artifact")]
    Show { task: String, name: String },
    #[command(about = "Prove an Artifact import without copying payload")]
    Preflight {
        task: String,
        name: String,
        #[arg(long)]
        source: PathBuf,
    },
    #[command(about = "Import a file or tree into one Artifact seat")]
    Import {
        task: String,
        name: String,
        #[arg(long)]
        source: PathBuf,
    },
    #[command(about = "Remove one exact Artifact seat")]
    Remove {
        task: String,
        name: String,
        #[arg(long)]
        apply: bool,
    },
}

impl Command {
    pub(crate) fn activity(&self) -> (&'static str, Vec<&str>) {
        match self {
            Self::List { task } => ("artifact.list", vec![task]),
            Self::Show { task, .. } => ("artifact.show", vec![task]),
            Self::Preflight { task, .. } => ("artifact.preflight", vec![task]),
            Self::Import { task, .. } => ("artifact.import", vec![task]),
            Self::Remove { task, .. } => ("artifact.remove", vec![task]),
        }
    }
}
