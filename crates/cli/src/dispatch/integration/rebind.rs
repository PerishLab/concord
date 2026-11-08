use super::{emit, observe, select};
use crate::args::integration::Rebinding;
use crate::runtime::input;
use concord_core::{Error, Estate, Integration, Rebind, Repository, Result};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u64,
    plan: Rebind,
}

pub(super) async fn run(estate: &Estate, command: Rebinding, output: bool) -> Result<()> {
    match command {
        Rebinding::Prepare {
            repository,
            node,
            key,
            path,
            command,
            timeout,
        } => {
            let held = select(estate, &repository, &node, key).await?;
            confirm(&held, &command, timeout).await?;
            let plan = estate.relocation(&held, &path).await?;
            emit(json!({"plan": plan}), output)
        }
        Rebinding::Apply {
            repository,
            input: source,
            apply,
            command,
            timeout,
        } => {
            let envelope: Envelope = input::read(&source, r#"{"version":1,"plan":{}}"#)?;
            if envelope.version != 1
                || envelope.plan.integration.repository != Repository::parse(&repository)?
            {
                return Err(changed());
            }
            estate.rebind(&envelope.plan, false).await?;
            confirm(&envelope.plan.integration, &command, timeout).await?;
            let integration = estate.rebind(&envelope.plan, apply).await?;
            emit(result(integration, apply), output)
        }
    }
}

async fn confirm(held: &Integration, command: &Path, timeout: u64) -> Result<()> {
    let current = observe(&held.repository, command, timeout).await?;
    if current.node != held.node {
        return Err(changed());
    }
    Ok(())
}

fn changed() -> Error {
    Error::typed(
        "concord.integration.changed",
        "rebind requires the exact current repository identity and a supported preparation envelope",
    )
}

fn result(integration: Integration, applied: bool) -> serde_json::Value {
    json!({"rebind": {"integration": integration, "applied": applied}})
}
