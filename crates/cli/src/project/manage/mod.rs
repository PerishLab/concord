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
use concord_core::{Error, Result};
use model::Plan;
pub(super) use model::{Action, Intent};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
                return Err(changed(&first, &current));
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
        Command::Show(_) | Command::Next(_) => unreachable!("independent observation dispatch"),
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

const CATEGORIES: &[&str] = &[
    "project",
    "fields",
    "items",
    "views",
    "workflows",
    "repositories",
    "statusUpdates",
    "columns",
    "origin",
    "repository",
    "issues",
    "owner",
    "projects",
];

fn changed(before: &Value, current: &Value) -> Error {
    let mut changed = CATEGORIES
        .iter()
        .copied()
        .filter(|name| before.get(*name) != current.get(*name))
        .collect::<Vec<_>>();
    if outside(before, current) || outside(current, before) {
        changed.push("other");
    }
    let truncated = changed.len() > 8;
    changed.truncate(8);
    let summary = changed.join(", ");
    let suffix = if truncated {
        ", additional categories omitted"
    } else {
        ""
    };
    Error::detailed(
        "concord.project.changed",
        format!(
            "native facts changed during preparation; observe again. Changed categories: {summary}{suffix}"
        ),
        json!({"changed":changed,"truncated":truncated}),
    )
}

fn outside(value: &Value, other: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return true;
    };
    object
        .iter()
        .any(|(key, value)| !CATEGORIES.contains(&key.as_str()) && other.get(key) != Some(value))
}

#[cfg(test)]
mod tests {
    use super::{CATEGORIES, changed};
    use serde_json::json;

    #[test]
    fn redacted() {
        let before = json!({"project":{"title":"old"},"items":[],"statusUpdates":[]});
        let current = json!({"project":{"title":"PRIVATE_TITLE"},"items":[{"id":"PRIVATE_ID"}],"statusUpdates":[{"body":"PRIVATE_REASON"}]});
        let error = changed(&before, &current);
        assert_eq!(error.code(), "concord.project.changed");
        assert_eq!(
            error.details().unwrap(),
            &json!({"changed":["project","items","statusUpdates"],"truncated":false})
        );
        assert!(!format!("{error:?}").contains("PRIVATE"));
    }

    #[test]
    fn bounded() {
        let mut current = serde_json::Map::new();
        for name in CATEGORIES {
            current.insert((*name).into(), json!("PRIVATE_VALUE"));
        }
        current.insert("PRIVATE_PATH".into(), json!({"body":"PRIVATE_REASON"}));
        let error = changed(&json!({}), &current.into());
        let details = error.details().unwrap();
        assert_eq!(details["changed"].as_array().unwrap().len(), 8);
        assert_eq!(details["truncated"], true);
        assert!(error.message().contains("additional categories omitted"));
        assert!(!format!("{error:?}").contains("PRIVATE"));
    }

    #[test]
    fn unknown() {
        let error = changed(&json!({}), &json!({"fields":null,"PRIVATE_PATH":null}));
        assert_eq!(
            error.details().unwrap()["changed"],
            json!(["fields", "other"])
        );
        assert!(!format!("{error:?}").contains("PRIVATE"));
        let error = changed(&json!("PRIVATE_OLD"), &json!(["PRIVATE_NEW"]));
        assert_eq!(
            error.details().unwrap(),
            &json!({"changed":["other"],"truncated":false})
        );
        assert!(!format!("{error:?}").contains("PRIVATE"));
    }
}
