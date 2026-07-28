mod args;
mod config;
mod dispatch;
mod output;
mod skill;

use args::Cli;
use clap::Parser;

fn main() {
    if let Err(error) = dispatch::run(Cli::parse()) {
        eprintln!("concord: {error}");
        std::process::exit(1);
    }
}
