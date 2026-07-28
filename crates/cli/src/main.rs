mod args;
mod dispatch;
mod output;

use args::Cli;
use clap::Parser;

fn main() {
    if let Err(error) = dispatch::run(Cli::parse()) {
        eprintln!("concord: {error}");
        std::process::exit(1);
    }
}
