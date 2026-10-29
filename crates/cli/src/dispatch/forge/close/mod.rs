pub(in crate::dispatch::forge) mod model;
mod read;

use crate::args::close::{Command, Read};
use concord_core::{Coordinate, Error, Result};
use model::{Envelope, Plan, State};
use read::Reader;
use serde_json::{Value, json};

const SEMANTICS: &str = "explicit operator action, not automatic judgment or lifecycle; no atomic body/review CAS; observed state does not establish authorship; unknown writes require read-only recovery before any new explicit action";

pub(in crate::dispatch) async fn run(command: Command, output: bool) -> Result<()> {
    let value = match command {
        Command::Prepare(args) => {
            let reader = Reader {
                coordinate: Coordinate::parse(&args.issue)?,
                args: &args,
            };
            let basis = reader.observe().await?.basis()?;
            if basis.issue.state != State::Open || !basis.issue.writable {
                return Err(fault(
                    "precondition",
                    "closure preparation requires an open Issue with current update capability",
                ));
            }
            json!({"version":1,"plan":Plan { schema:"concord.issue-close-plan/v1".into(), issue:reader.coordinate, basis }})
        }
        Command::Apply {
            read,
            apply: explicit,
        } => {
            crate::dispatch::explicit(explicit, "Issue lifecycle close").map_err(untouched)?;
            apply(read).await?
        }
        Command::Recover(args) => recover(args).await?,
    };
    crate::dispatch::emit(value, output)
}

async fn apply(args: Read) -> Result<Value> {
    let plan: Envelope = crate::dispatch::input::read(&args.input, "prepared Issue closure plan")
        .map_err(untouched)?;
    let reader = Reader {
        coordinate: Coordinate::parse(&args.observe.issue).map_err(untouched)?,
        args: &args.observe,
    };
    plan.validate(&reader.coordinate).map_err(untouched)?;
    let before = reader
        .observe()
        .await
        .map_err(untouched)?
        .basis()
        .map_err(untouched)?;
    if before.issue.completed() && plan.plan.contains(&before) {
        return Ok(
            json!({"closure":{"schema":"concord.issue-close-result/v1","outcome":"observed-closed","attempted":false,"current":before,"semantics":SEMANTICS}}),
        );
    }
    if before != plan.plan.basis {
        return Err(untouched(fault(
            "stale",
            "current Issue or acceptance differs from the prepared basis; no write was attempted; prepare from current facts",
        )));
    }
    reader
        .close(&before.issue)
        .await
        .map_err(|error| unknown(error, "request"))?;
    let current = reader
        .observe()
        .await
        .map_err(|error| unknown(error, "confirmation"))?;
    let basis = current
        .basis()
        .map_err(|error| unknown(error, "acceptance"))?;
    if !basis.issue.completed() || !plan.plan.contains(&basis) {
        return Err(unknown(
            fault(
                "changed",
                "lifecycle or acceptance basis differs after the close request",
            ),
            "confirmation",
        ));
    }
    Ok(
        json!({"closure":{"schema":"concord.issue-close-result/v1","outcome":"confirmed-closed","attempted":true,"current":current,"semantics":SEMANTICS}}),
    )
}

async fn recover(args: Read) -> Result<Value> {
    let plan: Envelope = crate::dispatch::input::read(&args.input, "prepared Issue closure plan")?;
    let reader = Reader {
        coordinate: Coordinate::parse(&args.observe.issue)?,
        args: &args.observe,
    };
    plan.validate(&reader.coordinate)?;
    let current = reader.observe().await?;
    let same = current
        .basis()
        .is_ok_and(|basis| plan.plan.contains(&basis));
    let outcome = if same && current.issue.completed() {
        "observed-closed"
    } else if current.issue.state == State::Open {
        "observed-open"
    } else {
        "unconfirmed"
    };
    Ok(
        json!({"closure":{"schema":"concord.issue-close-recovery/v1","outcome":outcome,"attempted":false,"basis_matches":same,"current":current,"semantics":SEMANTICS}}),
    )
}

fn fault(kind: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.issue.close.{kind}"), message)
}

fn unknown(error: Error, stage: &str) -> Error {
    Error::detailed(
        "concord.issue.close.indeterminate",
        "Issue close was attempted but its exact lifecycle and acceptance are not confirmed; run issue close recover with the retained plan before choosing another action",
        json!({"outcome":"unknown","attempted":true,"stage":stage,"cause_code":error.code(),"semantics":SEMANTICS}),
    )
}

fn untouched(error: Error) -> Error {
    Error::detailed(
        error.code(),
        error.message(),
        json!({"outcome":"not-attempted","attempted":false,"cause":error.details()}),
    )
}
