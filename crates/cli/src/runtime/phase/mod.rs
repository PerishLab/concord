mod args;
mod change;
mod model;
mod read;
mod schema;
mod wire;

pub(crate) use args::Command;
use concord_core::{Error, Result};
pub(crate) use model::Binding;
use serde_json::{Value, json};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) async fn observe(
    binding: Option<&Binding>,
    target: &str,
    node: &str,
    remote: (&Path, u64),
) -> Value {
    let Some(binding) = binding else {
        return json!({"status":"not-configured","observed_at":now(),"value":null,"semantics":"no explicit native Phase binding; unset is not Clarify"});
    };
    let client = wire::Client {
        command: remote.0,
        timeout: remote.1,
    };
    let result = read::snapshot(&client, binding, target)
        .await
        .and_then(|state| {
            if state["issue"]["id"] != node {
                return Err(fault(
                    "unavailable",
                    "Issue identity changed during Phase observation",
                ));
            }
            let value = read::value(binding, &state["issue"])?;
            let option = value.as_ref().map(|name| &binding.options[name]);
            let status = if value.is_some() { "set" } else { "unset" };
            Ok(json!({"status":status,"value":value,"option":option,"field":state["field"]}))
        });
    let mut value = match result {
        Ok(value) => value,
        Err(error) => {
            let status = if error.code() == "concord.phase.invalid" {
                "invalid"
            } else {
                "unavailable"
            };
            json!({"status":status,"value":null,"reason":error.message()})
        }
    };
    value["binding"] = json!(binding);
    value["observed_at"] = json!(now());
    value["authority"] = json!("issue");
    value
}

pub(crate) async fn run(command: Command, binding: Option<&Binding>, output: bool) -> Result<()> {
    let binding = binding.ok_or_else(|| fault("binding", "configure [phase] with owner login, organization node, field node and [phase.options] native IDs for Clarify/Deliver/Verify/Settle; references only, no Phase values/history"))?;
    let result = match command {
        Command::Prepare {
            issue,
            input,
            remote,
        } => {
            let intent: model::Intent = crate::runtime::input::read(
                &input,
                "Phase intent: reason and explicit value (null clears)",
            )?;
            let client = wire::Client {
                command: &remote.command,
                timeout: remote.timeout,
            };
            let before = read::snapshot(&client, binding, &issue).await?;
            change::qualify(binding, &intent, &before)?;
            let current = read::snapshot(&client, binding, &issue).await?;
            if before != current {
                return Err(fault(
                    "stale",
                    "native facts changed while preparing Phase; observe again",
                ));
            }
            json!({"plan":model::Plan {schema:"concord.issue-phase/v1".into(),issue,binding:binding.clone(),before,intent}})
        }
        Command::Apply {
            input,
            apply,
            remote,
        } => {
            crate::dispatch::explicit(apply, "Issue Phase mutation")?;
            let plan = plan(&input, binding)?;
            let client = wire::Client {
                command: &remote.command,
                timeout: remote.timeout,
            };
            change::apply(&client, &plan).await?
        }
        Command::Recover { input, remote } => {
            let plan = plan(&input, binding)?;
            let client = wire::Client {
                command: &remote.command,
                timeout: remote.timeout,
            };
            let current = read::snapshot(&client, binding, &plan.issue).await?;
            let observed = read::value(binding, &current["issue"])?;
            let status = if observed == plan.intent.value {
                "observed"
            } else {
                "unconfirmed"
            };
            json!({"desired_state":status,"current":current,"semantics":"read-only recovery; current state does not prove authorship; no mutation replayed"})
        }
    };
    crate::dispatch::emit(json!({"phase":result}), output)
}

fn plan(path: &Path, binding: &Binding) -> Result<model::Plan> {
    let value: Value = crate::runtime::input::read(path, "prepared Phase envelope")?;
    let plan: model::Plan = serde_json::from_value(value["phase"]["plan"].clone())
        .map_err(|_| fault("plan", "expected prepared phase.plan"))?;
    if plan.schema != "concord.issue-phase/v1" || plan.binding != *binding {
        return Err(fault("plan", "Phase plan schema or native binding changed"));
    }
    Ok(plan)
}

fn fault(code: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.phase.{code}"), message)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
