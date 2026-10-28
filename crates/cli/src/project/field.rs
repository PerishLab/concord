use super::{fault, wire::Client};
use concord_core::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Deserialize, Serialize)]
pub(super) enum Data {
    #[serde(rename = "TEXT")]
    Text,
    #[serde(rename = "NUMBER")]
    Number,
    #[serde(rename = "DATE")]
    Date,
    #[serde(rename = "SINGLE_SELECT")]
    Single,
    #[serde(rename = "MULTI_SELECT")]
    Multiple,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Choice {
    pub id: Option<String>,
    pub name: String,
    pub color: String,
    pub description: String,
}

pub(super) fn preserve(current: &Value, options: &[Choice]) -> Result<()> {
    choices(options)?;
    let key = match current["dataType"].as_str() {
        Some("SINGLE_SELECT") => "options",
        Some("MULTI_SELECT") => "multiSelectOptions",
        _ => {
            return Err(fault(
                "type",
                "only native select fields accept option configuration",
            ));
        }
    };
    let current = current[key]
        .as_array()
        .ok_or_else(|| fault("schema", "existing option identities are unreadable"))?;
    for option in options {
        if let Some(id) = &option.id
            && !current.iter().any(|prior| prior["id"] == *id)
        {
            return Err(fault(
                "identity",
                "option identity belongs to no current option of this exact field",
            ));
        }
    }
    for option in current {
        if !options
            .iter()
            .any(|next| next.id.as_deref() == option["id"].as_str())
        {
            return Err(fault(
                "identity",
                "option updates must preserve every existing native option identity",
            ));
        }
    }
    Ok(())
}

pub(super) fn choices(options: &[Choice]) -> Result<()> {
    if options.len() > 100 {
        return Err(fault("bounds", "field exceeds one hundred select options"));
    }
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for option in options {
        text(&option.name, 255)?;
        limit(&option.description, 1024)?;
        if !names.insert(&option.name) {
            return Err(fault("intent", "select option name repeated"));
        }
        if let Some(id) = &option.id
            && (id.is_empty() || !ids.insert(id))
        {
            return Err(fault(
                "identity",
                "select option identity is empty or repeated",
            ));
        }
        if !matches!(
            option.color.as_str(),
            "GRAY" | "BLUE" | "GREEN" | "YELLOW" | "ORANGE" | "RED" | "PINK" | "PURPLE"
        ) {
            return Err(fault("type", "unsupported native select option color"));
        }
    }
    Ok(())
}

pub(super) fn text(value: &str, maximum: usize) -> Result<()> {
    if value.trim().is_empty() {
        return Err(fault("intent", "required native text is empty"));
    }
    limit(value, maximum)
}

pub(super) fn limit(value: &str, maximum: usize) -> Result<()> {
    if value.len() > maximum {
        return Err(fault("bounds", "text exceeds the supported native bound"));
    }
    Ok(())
}

pub(super) fn empty(state: &Value, id: &str) -> Result<()> {
    let field = owned(state, id)?;
    if field["dataType"] == "ITERATION" {
        return Err(fault(
            "history",
            "generic empty-field removal cannot erase native cycle definitions",
        ));
    }
    for item in state["items"].as_array().expect("observed Project items") {
        let connection = &item["fieldValues"];
        let Some(values) = connection["nodes"].as_array() else {
            return Err(fault(
                "incomplete",
                "item field values are unreadable; no field is removed",
            ));
        };
        if item["content"].is_null() || connection["pageInfo"]["hasNextPage"] != false {
            return Err(fault(
                "incomplete",
                "item or field-value coverage is unknown; no field is removed",
            ));
        }
        for value in values {
            if !value["field"]["id"].is_string() {
                return Err(fault(
                    "schema",
                    "unknown field-value origin prevents empty-field removal",
                ));
            }
            if value["field"]["id"] == id {
                return Err(fault(
                    "populated",
                    "field still has native values; preserve its data and history",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn owned<'a>(state: &'a Value, id: &str) -> Result<&'a Value> {
    let field = state["fields"]
        .as_array()
        .and_then(|fields| fields.iter().find(|field| field["id"] == id))
        .ok_or_else(|| {
            fault(
                "identity",
                "selected field is not in this qualified Project",
            )
        })?;
    if field["authority"] != "project" {
        return Err(fault(
            "ownership",
            "operation requires an exact Project-owned field; Issue and derived fields are read-only",
        ));
    }
    Ok(field)
}

pub(in crate::project) async fn origin(
    client: &Client<'_>,
    project: &Value,
    intent: &super::manage::Intent,
) -> Result<Value> {
    let super::manage::Action::Attach { origin } = &intent.action else {
        unreachable!("attachment intent")
    };
    let data = client
        .read(ORIGIN, json!({"owner":project["owner"]["login"]}))
        .await?;
    let owner = &data["organization"];
    if owner["id"] != project["owner"]["id"] {
        return Err(fault(
            "identity",
            "Issue field organization differs from Project owner",
        ));
    }
    let fields = &owner["issueFields"];
    let nodes = fields["nodes"]
        .as_array()
        .ok_or_else(|| fault("unavailable", "organization Issue fields are unreadable"))?;
    if fields["pageInfo"]["hasNextPage"] != false
        || fields["totalCount"].as_u64() != Some(nodes.len() as u64)
        || nodes.iter().any(Value::is_null)
    {
        return Err(fault(
            "incomplete",
            "organization Issue field coverage is incomplete; no attachment prepared",
        ));
    }
    let found = nodes
        .iter()
        .filter(|field| field["id"] == *origin)
        .collect::<Vec<_>>();
    let [field] = found.as_slice() else {
        return Err(fault(
            "identity",
            "exact field is not a visible Issue field of the Project organization",
        ));
    };
    if field["__typename"] != "IssueFieldSingleSelect"
        || field["dataType"] != "SINGLE_SELECT"
        || field["visibility"] != "ORG_ONLY"
    {
        return Err(fault(
            "ownership",
            "attachment requires an organization-only native single-select Issue field",
        ));
    }
    Ok((*field).clone())
}

const ORIGIN: &str = "query($owner:String!){organization(login:$owner){id issueFields(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{__typename ... on IssueFieldSingleSelect{id fullDatabaseId name dataType visibility options{id name color description}} ... on IssueFieldText{id} ... on IssueFieldNumber{id} ... on IssueFieldDate{id} ... on IssueFieldMultiSelect{id}}}}}";
