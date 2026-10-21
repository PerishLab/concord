use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(
        about = "Read an explicit organization Project without opening local execution state",
        long_about = "Read an organization Project without opening local execution state, selected by OWNER/NUMBER or https://github.com/orgs/OWNER/projects/NUMBER. The result preserves native field ownership, item order, Issue facts and provider cursors. Root connections page within the selected budget; item pages are capped at 20 to keep their nested native projection within GitHub node limits; nested connections are bounded to 100 and retain their continuation cursors. Unknown content, incomplete collections and inaccessible automation details are explicit. This is a live interval, never an atomic snapshot, delegation or write authorization."
    )]
    Show {
        project: String,
        #[arg(long = "github-command")]
        command: PathBuf,
        #[arg(long = "page-size", default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=100))]
        size: u16,
        #[arg(long = "max-pages", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100), help = "Maximum pages per root connection")]
        pages: u16,
        #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60), help = "Timeout in seconds for each provider exchange")]
        timeout: u64,
    },
}
