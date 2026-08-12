use clap::Subcommand;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Survey exact legacy Space evidence")]
    Survey,
    #[command(about = "Build and verify a staged v0.10.0 estate")]
    Stage,
    #[command(about = "Reopen and verify an exact staged estate")]
    Resume { fingerprint: String },
    #[command(about = "Fence legacy registries and activate the staged estate")]
    Activate {
        fingerprint: String,
        #[arg(long)]
        apply: bool,
    },
}
