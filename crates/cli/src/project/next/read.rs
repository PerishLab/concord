use super::super::{
    fault, inspect,
    target::Target,
    wire::{self, Client},
};
use crate::args::project::Observe;
use crate::runtime::github;
use concord_core::Result;
use runseal::github::board::{Detail, Section};
use serde_json::{Value, json};

pub(super) async fn state(client: &Client, args: &Observe) -> Result<Value> {
    let target = Target::parse(&args.project)?;
    let project = &client.project(&target).await?;
    if project["owner"]["id"].as_str().is_none_or(str::is_empty) {
        return Err(fault(
            "identity",
            "Project organization identity is unreadable",
        ));
    }
    if !project["closed"].is_boolean() {
        return Err(fault("schema", "Project lifecycle is unreadable"));
    }
    let mut state = json!({"project":project});
    for (name, section, bound) in [
        ("fields", Section::Fields, args.size.min(20)),
        ("items", Section::Items(Detail::Next), args.size),
    ] {
        let mut collection = wire::collect(client, project, section, (bound, args.pages)).await?;
        if !collection.complete {
            return Err(fault(
                "incomplete",
                format!("{name} coverage is incomplete; next cannot decide"),
            ));
        }
        match name {
            "fields" => inspect::fields(&mut collection.nodes, project)?,
            "items" => inspect::membership(&collection.nodes, project)?,
            _ => unreachable!("next connections"),
        }
        state[name] = json!(collection.nodes);
    }
    Ok(state)
}

pub(super) async fn confirm(client: &Client, issue: &Value) -> Result<()> {
    let native = client.open()?;
    let id = issue["id"]
        .as_str()
        .ok_or_else(|| fault("identity", "selected Issue identity is unreadable"))?;
    let observed = github::call("project.confirm", || native.confirm(id))
        .map_err(wire::provider)?
        .value;
    if observed != *issue {
        return Err(fault(
            "changed",
            "selected Issue conditions changed or became inaccessible; read again",
        ));
    }
    Ok(())
}

pub(super) async fn finish(client: &Client, project: &Value) -> Result<()> {
    let current = client
        .metadata(project["id"].as_str().expect("qualified Project"))
        .await?;
    if current != *project {
        return Err(fault(
            "changed",
            "Project planning changed or became inaccessible before next completed",
        ));
    }
    Ok(())
}
