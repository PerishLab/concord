use super::fault;
use concord_core::Result;
use serde_json::{Value, json};

pub(super) fn fields(fields: &mut [Value], project: &Value) -> Result<()> {
    for field in fields {
        let kind = field["__typename"].as_str().unwrap_or("");
        if !matches!(
            kind,
            "ProjectV2Field"
                | "ProjectV2SingleSelectField"
                | "ProjectV2IterationField"
                | "ProjectV2MultiSelectField"
        ) {
            return Err(fault("schema", "unsupported Project field type"));
        }
        if field["project"]["id"] != project["id"]
            || !field["name"].is_string()
            || !field["dataType"].is_string()
        {
            return Err(fault(
                "schema",
                "unsupported or foreign Project field configuration",
            ));
        }
        shape(field)?;
        let attached = field["isIssueField"]
            .as_bool()
            .ok_or_else(|| fault("schema", "field ownership is unreadable"))?;
        if attached && field["issueField"]["id"].as_str().is_none_or(str::is_empty) {
            return Err(fault(
                "schema",
                "attached Issue field has no native origin identity",
            ));
        }
        field["authority"] = json!(authority(field, attached)?);
    }
    Ok(())
}

pub(super) fn items(items: &mut [Value], project: &Value) -> Result<()> {
    for item in items {
        if item["project"]["id"] != project["id"] || !item["isArchived"].is_boolean() {
            return Err(fault(
                "schema",
                "Project item identity or archival state is unreadable",
            ));
        }
        required(item, &["fieldValues"])?;
        let kind = item["content"]["__typename"]
            .as_str()
            .unwrap_or("")
            .to_string();
        item["custody"] = json!(match kind.as_str() {
            "Issue" if item["content"]["issueType"]["name"] == "Auto" => "automation-read-only",
            "Issue" if item["content"]["issueType"].is_null() => "unknown-read-only",
            "Issue" => "issue",
            "PullRequest" | "DraftIssue" => "non-issue-read-only",
            _ => "unknown-read-only",
        });
        if kind == "Issue" {
            validate(&item["content"])?;
        }
    }
    Ok(())
}

fn validate(issue: &Value) -> Result<()> {
    required(
        issue,
        &["blockedBy", "blocking", "subIssues", "labels", "comments"],
    )?;
    if issue["id"].as_str().is_none_or(str::is_empty)
        || issue["repository"]["id"].as_str().is_none_or(str::is_empty)
    {
        return Err(fault("schema", "native Issue identity is unreadable"));
    }
    if !issue["body"].is_string() || !matches!(issue["state"].as_str(), Some("OPEN" | "CLOSED")) {
        return Err(fault(
            "schema",
            "native Issue lifecycle or acceptance body is unreadable",
        ));
    }
    if issue["number"].as_u64().is_none_or(|number| number == 0)
        || issue["repository"]["nameWithOwner"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        return Err(fault("schema", "native Issue coordinate is unreadable"));
    }
    Ok(())
}

pub(super) fn gaps(value: &Value, path: &str, diagnostics: &mut Vec<Value>) -> Result<()> {
    match value {
        Value::Object(object) => {
            connection(value, path, diagnostics)?;
            for (key, child) in object {
                gaps(child, &format!("{path}/{key}"), diagnostics)?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                gaps(child, &format!("{path}/{index}"), diagnostics)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn connection(value: &Value, path: &str, diagnostics: &mut Vec<Value>) -> Result<()> {
    let Some(page) = value.get("pageInfo") else {
        return Ok(());
    };
    let nodes = value["nodes"]
        .as_array()
        .ok_or_else(|| fault("schema", format!("unreadable nested connection at {path}")))?;
    let total = value["totalCount"].as_u64();
    if total.is_none() && !path.ends_with("/configuration/visibleFields") {
        return Err(fault("schema", format!("missing nested total at {path}")));
    }
    let more = page["hasNextPage"]
        .as_bool()
        .ok_or_else(|| fault("schema", format!("missing nested continuation at {path}")))?;
    if more && page["endCursor"].as_str().is_none_or(str::is_empty) {
        return Err(fault("schema", format!("missing nested cursor at {path}")));
    }
    if nodes.iter().any(Value::is_null)
        || (!more && total.is_some_and(|total| nodes.len() as u64 != total))
    {
        diagnostics.push(json!({"code": "unreadable", "path": path, "provider_total": total}));
    }
    if more {
        diagnostics.push(json!({"code": "nested-budget", "path": path, "end_cursor": page["endCursor"], "provider_total": total}));
    }
    Ok(())
}

pub(super) fn unknown(items: &[Value], diagnostics: &mut Vec<Value>) {
    for item in items {
        if item["custody"] == "unknown-read-only" {
            diagnostics.push(json!({"code": "unknown-content-or-type", "item": item["id"]}));
        }
        if item["content"]["issueFields"]["complete"] == false {
            diagnostics.push(json!({"code": "issue-field-budget", "item": item["id"]}));
        }
        values(item, diagnostics);
    }
}

fn values(item: &Value, diagnostics: &mut Vec<Value>) {
    let Some(values) = item["fieldValues"]["nodes"].as_array() else {
        return;
    };
    for value in values {
        if !value
            .get("field")
            .is_some_and(|field| field["id"].is_string())
        {
            diagnostics.push(json!({"code": "unsupported-value", "item": item["id"], "typename": value["__typename"]}));
        }
    }
}

fn required(value: &Value, names: &[&str]) -> Result<()> {
    for name in names {
        if !value[*name]["pageInfo"].is_object() || !value[*name]["nodes"].is_array() {
            return Err(fault(
                "schema",
                format!("required native connection {name} is unreadable"),
            ));
        }
    }
    Ok(())
}

fn shape(field: &Value) -> Result<()> {
    let expected = match field["dataType"].as_str() {
        Some("SINGLE_SELECT") => "ProjectV2SingleSelectField",
        Some("MULTI_SELECT") => "ProjectV2MultiSelectField",
        Some("ITERATION") => "ProjectV2IterationField",
        _ => "ProjectV2Field",
    };
    if field["__typename"] != expected {
        return Err(fault(
            "schema",
            "native Project field type disagrees with its configuration",
        ));
    }
    if field["isIssueField"] == true && field["issueField"]["dataType"] != field["dataType"] {
        return Err(fault(
            "schema",
            "attached Issue field type disagrees with its origin",
        ));
    }
    Ok(())
}

fn authority(field: &Value, attached: bool) -> Result<&'static str> {
    if attached {
        let origin = &field["issueField"]["fullDatabaseId"];
        if origin
            .as_u64()
            .or_else(|| origin.as_str().and_then(|value| value.parse::<u64>().ok()))
            .is_none()
        {
            return Err(fault(
                "schema",
                "attached Issue field native database identity is unreadable",
            ));
        }
        return Ok("issue");
    }
    match field["dataType"].as_str() {
        Some("TEXT" | "SINGLE_SELECT" | "MULTI_SELECT" | "NUMBER" | "DATE" | "ITERATION") => {
            Ok("project")
        }
        Some(
            "TITLE"
            | "ASSIGNEES"
            | "LINKED_PULL_REQUESTS"
            | "REVIEWERS"
            | "LABELS"
            | "MILESTONE"
            | "REPOSITORY"
            | "TRACKS"
            | "TRACKED_BY"
            | "ISSUE_TYPE"
            | "PARENT_ISSUE"
            | "SUB_ISSUES_PROGRESS"
            | "CREATED"
            | "UPDATED"
            | "CLOSED",
        ) => Ok("native-projection"),
        _ => Err(fault(
            "schema",
            "unsupported native Project field data type",
        )),
    }
}

pub(super) fn configuration(nodes: &[Value], project: &Value, name: &str) -> Result<()> {
    for node in nodes {
        if node["project"]["id"] != project["id"] {
            return Err(fault("identity", "foreign Project configuration member"));
        }
        match name {
            "views" => view(node)?,
            "workflows" if !node["enabled"].is_boolean() => {
                return Err(fault(
                    "schema",
                    "native workflow enabled state is unreadable",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn view(node: &Value) -> Result<()> {
    required(
        node,
        &["groupByFields", "verticalGroupByFields", "sortByFields"],
    )?;
    required(&node["configuration"], &["visibleFields"])?;
    if !node["name"].is_string() || !node["layout"].is_string() {
        return Err(fault("schema", "native view configuration is unreadable"));
    }
    Ok(())
}
