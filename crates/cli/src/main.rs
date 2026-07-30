mod args;
mod config;
mod dispatch;
mod output;
mod skill;

use args::Cli;
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    if let Err(error) = dispatch::run(cli) {
        if json {
            eprintln!("{}", error_json(&error));
        } else {
            eprintln!("concord: {error}");
        }
        std::process::exit(1);
    }
}

fn error_json(error: &concord_core::Error) -> String {
    let body = serde_json::json!({
        "error": {
            "code": error.code(),
            "message": error.message(),
            "details": error.details(),
        }
    });
    serde_json::to_string(&body).expect("error JSON should encode")
}
