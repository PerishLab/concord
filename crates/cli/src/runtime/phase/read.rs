use super::{Binding, fault, schema, wire::Client};
use crate::runtime::github;
use concord_core::{Coordinate, Result};
use runseal::github::Target;
use serde_json::{Value, json};
use std::collections::BTreeSet;

struct Projection {
    full: bool,
    connections: &'static [&'static str],
}

pub(super) async fn snapshot(client: &Client, binding: &Binding, target: &str) -> Result<Value> {
    read(
        client,
        binding,
        target,
        Projection {
            full: true,
            connections: &[
                "issueFieldValues",
                "labels",
                "blockedBy",
                "blocking",
                "comments",
                "subIssues",
            ],
        },
    )
    .await
}

pub(super) async fn observe(client: &Client, binding: &Binding, target: &str) -> Result<Value> {
    read(
        client,
        binding,
        target,
        Projection {
            full: false,
            connections: &["issueFieldValues"],
        },
    )
    .await
}

async fn read(
    client: &Client,
    binding: &Binding,
    target: &str,
    projection: Projection,
) -> Result<Value> {
    let coordinate = Coordinate::parse(target)?;
    if coordinate.owner != binding.owner {
        return Err(fault(
            "invalid",
            "Issue is outside the bound Phase organization",
        ));
    }
    let schema = schema::read(client, binding).await?;
    let native = client.open()?;
    let number = u32::try_from(coordinate.number)
        .map_err(|_| fault("unavailable", "invalid Issue number"))?;
    let locator = Target::new(&coordinate.owner, &coordinate.repository, number)
        .map_err(super::wire::provider)?;
    let issue = github::call("phase.read", || native.phase(&locator, projection.full))
        .map_err(super::wire::provider)?
        .value;
    let url = format!(
        "https://github.com/{}/{}/issues/{}",
        coordinate.owner, coordinate.repository, coordinate.number
    );
    let identity = issue["id"].as_str().is_some_and(|id| !id.is_empty())
        && issue["repository"]["owner"]["id"] == binding.organization;
    let address = issue["url"] == url && issue["number"] == coordinate.number;
    if !identity || !address {
        return Err(fault(
            "unavailable",
            "exact native Issue identity is unavailable",
        ));
    }
    for &key in projection.connections {
        complete(&issue[key])?;
    }
    let mut fields = BTreeSet::new();
    for value in nodes(&issue["issueFieldValues"])? {
        let field = value["field"]["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| fault("unavailable", "native field value origin is unreadable"))?;
        if value["id"].as_str().is_none_or(str::is_empty) || !fields.insert(field) {
            return Err(fault(
                "unavailable",
                "native field value identity is missing or repeated",
            ));
        }
    }
    value(binding, &issue)?;
    Ok(json!({"field":schema,"issue":issue}))
}

pub(super) fn value(binding: &Binding, issue: &Value) -> Result<Option<String>> {
    let values = nodes(&issue["issueFieldValues"])?;
    let selected = values
        .iter()
        .filter(|value| value["field"]["id"] == binding.field)
        .collect::<Vec<_>>();
    let value = match selected.as_slice() {
        [] => return Ok(None),
        [value] => value,
        _ => return Err(fault("invalid", "Phase field has multiple native values")),
    };
    if value["__typename"] != "IssueFieldSingleSelectValue" {
        return Err(fault(
            "invalid",
            "Phase value is not a native single-select",
        ));
    }
    binding
        .options
        .iter()
        .find(|(name, id)| value["optionId"] == **id && value["name"] == **name)
        .map(|(name, _)| Some(name.clone()))
        .ok_or_else(|| fault("invalid", "Phase value refers to an unknown option"))
}

pub(super) fn nodes(connection: &Value) -> Result<&Vec<Value>> {
    let nodes = connection["nodes"]
        .as_array()
        .ok_or_else(|| fault("unavailable", "native connection is unreadable"))?;
    if nodes.iter().any(Value::is_null) || !connection["pageInfo"]["hasNextPage"].is_boolean() {
        return Err(fault(
            "unavailable",
            "native connection contains unknown facts",
        ));
    }
    Ok(nodes)
}

fn complete(connection: &Value) -> Result<()> {
    let values = nodes(connection)?;
    if connection["pageInfo"]["hasNextPage"] != false
        || connection["totalCount"].as_u64() != Some(values.len() as u64)
    {
        return Err(fault(
            "unavailable",
            "native Issue facts are incomplete; no empty value can be inferred",
        ));
    }
    Ok(())
}
