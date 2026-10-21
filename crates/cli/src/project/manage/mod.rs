mod apply;
mod build;
mod desired;
mod journal;
mod model;
mod native;
mod qualify;
mod read;
mod rest;

use super::{fault, wire::Client};
use crate::args::project::{Command, Remote};
use crate::runtime::input;
pub(super) use build::Step;
use concord_core::Result;
use model::Plan;
pub(super) use model::{Action, Intent};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    plan: Plan,
}

pub(super) async fn run(command: Command, output: bool) -> Result<()> {
    let value = match command {
        Command::List { owner, remote } => {
            let current = read::inventory(&client(&remote), &owner, remote.pages).await?;
            json!({"schema":"concord.project-inventory/v1","complete":true,"inventory":current,"coverage":"provider-visible Projects; hidden or inaccessible resources are not proven absent"})
        }
        Command::Prepare {
            project,
            input: path,
            remote,
        } => {
            let intent: Intent = input::read(&path, "Project intent: reason and typed action")?;
            let client = client(&remote);
            let first = read::state(&client, &project, &intent, remote.pages).await?;
            build::steps(&first, &intent)?;
            let current = read::state(&client, &project, &intent, remote.pages).await?;
            if first != current {
                return Err(fault(
                    "changed",
                    "native facts changed during preparation; observe again",
                ));
            }
            let plan = Plan {
                schema: "concord.project-plan/v1".into(),
                target: project,
                before: current,
                intent,
            };
            let envelope = json!({"plan":plan,"version":1});
            if serde_json::to_vec(&envelope).expect("Project plan").len() > 1024 * 1024 {
                return Err(fault(
                    "bounds",
                    "prepared native context exceeds the bounded input envelope",
                ));
            }
            crate::output::value(envelope, output);
            return Ok(());
        }
        Command::Apply {
            input: path,
            apply: explicit,
            remote,
        } => {
            crate::dispatch::explicit(explicit, "Project provider mutation")?;
            let envelope: Envelope = input::read(&path, "prepared Project plan")?;
            validate(&envelope)?;
            apply::run(&client(&remote), &envelope.plan, remote.pages).await?
        }
        Command::Recover {
            input: path,
            remote,
        } => {
            let envelope: Envelope = input::read(&path, "prepared Project plan")?;
            validate(&envelope)?;
            apply::recover(&client(&remote), &envelope.plan, remote.pages).await?
        }
        Command::Show(_) => unreachable!("independent observation dispatch"),
    };
    crate::dispatch::emit(json!({"project":value}), output)
}

fn client(remote: &Remote) -> Client<'_> {
    Client {
        command: &remote.command,
        timeout: remote.timeout,
    }
}

fn validate(envelope: &Envelope) -> Result<()> {
    if envelope.version != 1 || envelope.plan.schema != "concord.project-plan/v1" {
        return Err(fault("schema", "unsupported prepared Project plan version"));
    }
    Ok(())
}
