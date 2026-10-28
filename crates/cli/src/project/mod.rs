mod field;
mod inspect;
mod issue;
mod manage;
mod next;
mod observe;
mod planning;
use wire::query;
mod target;
mod wire;

use crate::args::project::Command;
use concord_core::{Error, Result};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) async fn run(
    command: Command,
    output: bool,
    phase: Option<&crate::phase::Binding>,
) -> Result<()> {
    match command {
        Command::Show(args) => observe::run(args, output).await,
        Command::Next(args) => next::run(args, output, phase).await,
        command => manage::run(command, output).await,
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fault(code: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.project.{code}"), message)
}
