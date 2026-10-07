use clap::{Args as Clap, Subcommand};
use std::path::PathBuf;

#[derive(Clap)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Read complete current acceptance labels and comment history")]
    Show(Observe),
    #[command(about = "Interpret acceptance against live review freshness without writes")]
    Evaluate(Observe),
    #[command(
        about = "Check current PR closing references against an explicit head without writes"
    )]
    Closing(Closing),
    #[command(about = "Prepare an Issue-bound declaration plan without provider writes")]
    Declare(Prepare),
    #[command(about = "Prepare an explicit predecessor amendment without provider writes")]
    Amend(Prepare),
    #[command(about = "Prepare a closure attestation without provider writes or Issue closing")]
    Judge(Prepare),
    #[command(about = "Apply or recover one prepared acceptance plan without closing the Issue")]
    Apply(Apply),
}

#[derive(Clap)]
pub struct Observe {
    pub issue: String,
    #[arg(long = "github-command", value_name = "COMMAND")]
    pub command: PathBuf,
    #[arg(long = "observe-timeout", default_value_t = 10, value_name = "SECONDS")]
    pub timeout: u64,
    #[arg(long = "max-pages", default_value_t = 20, value_name = "COUNT")]
    pub pages: usize,
}

#[derive(Clap)]
pub struct Prepare {
    #[command(flatten)]
    pub observe: Observe,
    #[arg(long, default_value = "-", value_name = "PATH|-")]
    pub input: PathBuf,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Show(_) => "acceptance.show",
            Self::Evaluate(_) => "acceptance.evaluate",
            Self::Closing(_) => "acceptance.closing",
            Self::Declare(_) => "acceptance.declare",
            Self::Amend(_) => "acceptance.amend",
            Self::Judge(_) => "acceptance.judge",
            Self::Apply(_) => "acceptance.apply",
        }
    }
}

#[derive(Clap)]
pub struct Apply {
    #[command(flatten)]
    pub prepare: Prepare,
    #[arg(long)]
    pub apply: bool,
}

#[derive(Clap)]
pub struct Closing {
    #[command(flatten)]
    pub observe: Observe,
    #[arg(long, value_name = "COMMIT")]
    pub head: String,
}
