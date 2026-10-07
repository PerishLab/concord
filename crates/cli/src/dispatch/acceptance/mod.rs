mod apply;
mod clock;
mod evaluate;
mod freshness;
mod labels;
mod model;
mod plan;
mod prepare;
mod read;

pub(super) use evaluate::read as review;

use crate::args::acceptance::{Command, Observe};
use concord_core::acceptance::History;
use concord_core::{Coordinate, Error, Result};
use serde_json::json;

pub async fn run(command: Command, output: bool) -> Result<()> {
    let value = match command {
        Command::Show(args) => {
            let snapshot = observe(&args).await?;
            let history = History::read(&snapshot.comments())?;
            let target = history.target(&snapshot.labels)?;
            show(snapshot, history, target)
        }
        Command::Declare(args) => prepare::run(args, prepare::Purpose::Declaration).await?,
        Command::Evaluate(args) => evaluate::run(args).await?,
        Command::Closing(args) => return evaluate::closing(args, output).await,
        Command::Amend(args) => prepare::run(args, prepare::Purpose::Amendment).await?,
        Command::Judge(args) => prepare::run(args, prepare::Purpose::Closure).await?,
        Command::Apply(args) => apply::run(args).await?,
    };
    super::emit(value, output)
}

fn show(
    snapshot: model::Snapshot,
    history: History,
    target: Option<concord_core::acceptance::Target>,
) -> serde_json::Value {
    json!({"acceptance": {"snapshot": snapshot, "history": history, "target": target}})
}

async fn observe(args: &Observe) -> Result<model::Snapshot> {
    if !(1..=20).contains(&args.pages) || !(1..=120).contains(&args.timeout) {
        return Err(fault(
            "bounds",
            "acceptance reads require 1..20 pages and a 1..120 second request timeout",
        ));
    }
    let coordinate = Coordinate::parse(&args.issue)?;
    let reader = read::Reader {
        coordinate: &coordinate,
        command: &args.command,
        timeout: args.timeout,
        pages: args.pages,
    };
    let first = reader.complete().await?;
    let second = reader.complete().await?;
    if first != second {
        return Err(fault(
            "changed",
            "Issue, labels or comments changed during acceptance observation",
        ));
    }
    Ok(second)
}

fn fault(kind: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.acceptance.{kind}"), message.into())
}
