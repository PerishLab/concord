use super::{
    super::{fault, planning},
    scope::Scope,
};
use concord_core::Result;
use serde_json::Value;

pub(super) fn select<'a>(
    items: &'a Value,
    scope: &Scope<'_>,
    project: &Value,
) -> Result<Option<&'a Value>> {
    let items = items
        .as_array()
        .ok_or_else(|| fault("schema", "native Project items are unreadable"))?;
    for item in items {
        let issue = &item["content"];
        if item["isArchived"] == true || issue["__typename"] != "Issue" {
            continue;
        }
        if !matches!(
            issue["issueType"]["name"].as_str(),
            Some("Task" | "Bug" | "Feature")
        ) {
            continue;
        }
        match issue["state"].as_str() {
            Some("CLOSED") => continue,
            Some("OPEN") => {}
            _ => return Err(fault("schema", "ordinary Issue lifecycle is unreadable")),
        }
        if planning::selected(item, scope.cycle, "iterationId")? != scope.period["id"].as_str() {
            continue;
        }
        identity(issue, project)?;
        let labels = connection(&issue["labels"])?;
        let blockers = connection(&issue["blockedBy"])?;
        let blocked = blockers
            .iter()
            .try_fold(false, |blocked, blocker| match blocker["state"].as_str() {
                Some("OPEN") => Ok(true),
                Some("CLOSED") => Ok(blocked),
                _ => Err(fault("schema", "blocker lifecycle is unreadable")),
            })?;
        let mut needs = false;
        for label in labels {
            let name = label["name"]
                .as_str()
                .ok_or_else(|| fault("schema", "Issue label is unreadable"))?;
            needs |= name.starts_with("needs:");
        }
        if !blocked && !needs {
            scope.priority(item)?;
            return Ok(Some(item));
        }
    }
    Ok(None)
}

fn connection(value: &Value) -> Result<&Vec<Value>> {
    let nodes = value["nodes"]
        .as_array()
        .ok_or_else(|| fault("incomplete", "Issue conditions are unreadable"))?;
    if value["pageInfo"]["hasNextPage"] != false
        || value["totalCount"].as_u64() != Some(nodes.len() as u64)
        || nodes.iter().any(Value::is_null)
    {
        return Err(fault(
            "incomplete",
            "Issue conditions are incomplete; next cannot decide",
        ));
    }
    Ok(nodes)
}

fn identity(issue: &Value, project: &Value) -> Result<()> {
    if issue["repository"]["owner"]["id"] != project["owner"]["id"] {
        return Err(fault(
            "identity",
            "selected Issue is outside the Project organization",
        ));
    }
    for key in ["id", "url", "title", "updatedAt"] {
        if issue[key].as_str().is_none_or(str::is_empty) {
            return Err(fault("schema", "selected Issue identity is unreadable"));
        }
    }
    Ok(())
}
