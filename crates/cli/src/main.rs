mod args;
mod config;
mod dispatch;
mod observation;
mod output;
mod skill;

use args::Cli;
use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    let observation = observation::Run::start(cli.command.name());
    let result = dispatch::run(cli).await;
    if let Some(observation) = observation {
        observation.finish(i32::from(result.is_err()));
    }
    if let Err(error) = result {
        if json {
            eprintln!("{}", failure(&error));
        } else {
            eprintln!("concord: {error}");
        }
        std::process::exit(1);
    }
}

fn failure(error: &concord_core::Error) -> String {
    let body = serde_json::json!({
        "error": {
            "code": error.code(),
            "message": error.message(),
            "details": error.details(),
        }
    });
    serde_json::to_string(&body).expect("error JSON should encode")
}
