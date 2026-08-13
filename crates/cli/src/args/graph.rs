use clap::Subcommand;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Return explicit forward and reverse adjacency")]
    Adjacency {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long = "min-weight", default_value = "unknown")]
        minimum: String,
        #[arg(long)]
        retired: bool,
    },
    #[command(about = "Traverse bounded neighbors from one Task")]
    Neighbors {
        task: String,
        #[arg(long, default_value = "out")]
        direction: String,
        #[arg(long, default_value_t = 1)]
        depth: usize,
        #[arg(long = "min-weight", default_value = "unknown")]
        minimum: String,
    },
    #[command(about = "Count direct incoming and outgoing dependencies")]
    Degree {
        task: String,
        #[arg(long = "min-weight", default_value = "unknown")]
        minimum: String,
    },
    #[command(about = "Traverse every reachable Task")]
    Reach {
        task: String,
        #[arg(long, default_value = "out")]
        direction: String,
        #[arg(long = "min-weight", default_value = "unknown")]
        minimum: String,
    },
    #[command(about = "Find the shortest directed dependency path")]
    Path {
        source: String,
        target: String,
        #[arg(long = "min-weight", default_value = "unknown")]
        minimum: String,
        #[arg(long, conflicts_with = "shortest")]
        all: bool,
        #[arg(long, conflicts_with = "all")]
        shortest: bool,
    },
    #[command(about = "Expose directed cycles")]
    Cycles {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        retired: bool,
    },
    #[command(about = "Expose strongly connected components")]
    Scc {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        retired: bool,
    },
    #[command(about = "Export the deterministic direct graph")]
    Export {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long = "min-weight", default_value = "unknown")]
        minimum: String,
        #[arg(long)]
        retired: bool,
        #[arg(long)]
        from: Option<String>,
        #[arg(long, requires = "from")]
        depth: Option<usize>,
    },
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Adjacency { .. } => "graph.adjacency",
            Self::Neighbors { .. } => "graph.neighbors",
            Self::Degree { .. } => "graph.degree",
            Self::Reach { .. } => "graph.reach",
            Self::Path { .. } => "graph.path",
            Self::Cycles { .. } => "graph.cycles",
            Self::Scc { .. } => "graph.scc",
            Self::Export { .. } => "graph.export",
        }
    }

    pub(crate) fn activity(&self) -> Option<Vec<&str>> {
        match self {
            Self::Neighbors { task, .. } => Some(vec![task]),
            Self::Degree { task, .. } => Some(vec![task]),
            Self::Reach { task, .. } => Some(vec![task]),
            Self::Path { source, target, .. } => Some(vec![source, target]),
            Self::Export { from, .. } => from.as_deref().map(|task| vec![task]),
            Self::Adjacency { .. } | Self::Cycles { .. } | Self::Scc { .. } => None,
        }
    }
}
