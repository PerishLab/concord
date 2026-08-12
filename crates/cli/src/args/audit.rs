#[derive(clap::Args)]
pub struct Args {
    pub task: Option<String>,
    #[arg(long)]
    pub domain: Option<String>,
}
