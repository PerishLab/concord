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
