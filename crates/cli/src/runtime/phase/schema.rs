use super::{Binding, fault, model::NAMES, wire::Client};
use concord_core::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) async fn read(client: &Client<'_>, binding: &Binding) -> Result<Value> {
    let mut after = Value::Null;
    let mut fields = Vec::new();
    let mut seen = BTreeSet::new();
    for _ in 0..20 {
        let data = client
            .query(QUERY, json!({"owner":binding.owner,"after":after}))
            .await?;
        let owner = &data["organization"];
        if owner.is_null() {
            return Err(fault("unavailable", "Phase organization is inaccessible"));
        }
        if owner["id"] != binding.organization || owner["login"] != binding.owner {
            return Err(fault(
                "invalid",
                "Phase organization is missing, inaccessible or different from its binding",
            ));
        }
        let connection = &owner["issueFields"];
        fields.extend(super::read::nodes(connection)?.iter().cloned());
        if connection["pageInfo"]["hasNextPage"] == false {
            if connection["totalCount"].as_u64() != Some(fields.len() as u64) {
                return Err(fault(
                    "unavailable",
                    "organization Issue field coverage changed",
                ));
            }
            return qualify(binding, &fields);
        }
        after = connection["pageInfo"]["endCursor"].clone();
        if !after.is_string() || !seen.insert(after.to_string()) {
            return Err(fault(
                "unavailable",
                "organization Issue field cursor does not advance",
            ));
        }
    }
    Err(fault(
        "unavailable",
        "organization Issue field coverage is incomplete",
    ))
}

fn qualify(binding: &Binding, fields: &[Value]) -> Result<Value> {
    let mut ids = BTreeSet::new();
    if binding.options.len() != 4
        || NAMES
            .iter()
            .any(|name| !binding.options.contains_key(*name))
    {
        return Err(fault(
            "invalid",
            "Phase binding requires exactly Clarify, Deliver, Verify and Settle option IDs",
        ));
    }
    let matches = fields
        .iter()
        .filter(|field| field["id"] == binding.field)
        .collect::<Vec<_>>();
    let [field] = matches.as_slice() else {
        return Err(fault(
            "invalid",
            "bound Phase field is not one exact organization Issue field",
        ));
    };
    if field["__typename"] != "IssueFieldSingleSelect"
        || field["dataType"] != "SINGLE_SELECT"
        || field["visibility"] != "ORG_ONLY"
    {
        return Err(fault(
            "invalid",
            "Phase requires an organization-only native single-select Issue field",
        ));
    }
    let options = field["options"]
        .as_array()
        .filter(|options| options.len() == 4)
        .ok_or_else(|| fault("invalid", "Phase option schema changed"))?;
    for name in NAMES {
        let id = &binding.options[name];
        if id.is_empty()
            || !ids.insert(id)
            || options
                .iter()
                .filter(|option| option["id"] == *id && option["name"] == name)
                .count()
                != 1
        {
            return Err(fault("invalid", "Phase option identity or meaning changed"));
        }
    }
    Ok((*field).clone())
}

const QUERY: &str = r#"query($owner:String!,$after:String){organization(login:$owner){id login issueFields(first:100,after:$after){totalCount pageInfo{hasNextPage endCursor} nodes{__typename ... on IssueFieldSingleSelect{id name dataType visibility options{id name}} ... on IssueFieldText{id} ... on IssueFieldNumber{id} ... on IssueFieldDate{id} ... on IssueFieldMultiSelect{id}}}}}"#;
