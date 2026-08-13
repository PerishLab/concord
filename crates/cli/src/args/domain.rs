use clap::Subcommand;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Bootstrap a fresh Keel estate and its first Domain")]
    Bootstrap { name: String },
    #[command(about = "List managed Domains")]
    List,
    #[command(about = "Add a Domain to the active estate")]
    Add { name: String },
    #[command(about = "Manage typed Repository annotations")]
    Repository {
        #[command(subcommand)]
        command: Repository,
    },
}

#[derive(Subcommand)]
pub enum Repository {
    #[command(about = "List Repository annotations")]
    List {
        #[arg(long)]
        domain: Option<String>,
    },
    #[command(about = "Create or replace one Repository annotation")]
    Annotate {
        domain: String,
        name: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        revision: i64,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Bootstrap { .. } => "domain.bootstrap",
            Self::List => "domain.list",
            Self::Add { .. } => "domain.add",
            Self::Repository { command } => command.name(),
        }
    }
}

impl Repository {
    fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "domain.repository.list",
            Self::Annotate { .. } => "domain.repository.annotate",
        }
    }
}
