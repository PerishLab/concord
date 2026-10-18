use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "List one Issue's direct filesystem Artifacts")]
    List { issue: String },
    #[command(about = "Resolve one direct Issue Artifact")]
    Show { issue: String, name: String },
    #[command(about = "Prove an Issue Artifact import without copying payload")]
    Preflight(Import),
    #[command(about = "Import a file or tree into one Issue Artifact seat")]
    Import(Import),
    #[command(about = "Remove one exact Issue Artifact seat")]
    Remove {
        issue: String,
        name: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(clap::Args)]
pub struct Import {
    pub issue: String,
    pub name: String,
    #[arg(long)]
    pub source: PathBuf,
    #[arg(long)]
    pub revision: i64,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "artifact.list",
            Self::Show { .. } => "artifact.show",
            Self::Preflight(_) => "artifact.preflight",
            Self::Import(_) => "artifact.import",
            Self::Remove { .. } => "artifact.remove",
        }
    }
    pub(crate) fn activity(&self) -> Vec<&str> {
        match self {
            Self::List { issue } | Self::Show { issue, .. } | Self::Remove { issue, .. } => {
                vec![issue]
            }
            Self::Preflight(import) | Self::Import(import) => vec![&import.issue],
        }
    }
}
