use clap::Subcommand;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Inventory an exact v0.13.0 source without changing it")]
    Inventory,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Inventory => "transition.inventory",
        }
    }
}
