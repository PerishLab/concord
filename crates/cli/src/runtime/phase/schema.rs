use super::{Binding, fault, model::NAMES, wire::Client};
use crate::runtime::github;
use concord_core::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) async fn read(client: &Client, binding: &Binding) -> Result<Value> {
    let client = client.open()?;
    let mut after: Option<String> = None;
    let mut expected = None;
    let mut fields = Vec::new();
    let mut seen = BTreeSet::new();
    for _ in 0..20 {
        let window =
            runseal::github::Window::new(100, after.as_deref()).map_err(super::wire::provider)?;
        let page = github::call("phase.definitions", || {
            client.definitions(&binding.owner, &window)
        })
        .map_err(super::wire::provider)?
        .value;
        if page.target != binding.organization {
            return Err(fault(
                "invalid",
                "Phase organization differs from its binding",
            ));
        }
        if *expected.get_or_insert(page.total) != page.total {
            return Err(fault(
                "unavailable",
                "organization Issue field coverage changed",
            ));
        }
        fields.extend(page.rows.into_iter().map(definition));
        match page.next {
            Some(next) if seen.insert(next.clone()) => after = Some(next),
            None if fields.len() == page.total as usize => return qualify(binding, &fields),
            _ => {
                return Err(fault(
                    "unavailable",
                    "organization Issue field coverage is incomplete",
                ));
            }
        }
    }
    Err(fault(
        "unavailable",
        "organization Issue field coverage is incomplete",
    ))
}

fn definition(field: runseal::github::phase::Definition) -> Value {
    let mut value = json!({"__typename":field.kind});
    if let Some(id) = field.id {
        value["id"] = json!(id);
    }
    if value["__typename"] == "IssueFieldSingleSelect" {
        value["name"] = json!(field.name);
        value["dataType"] = json!(field.datatype);
        value["visibility"] = json!(field.visibility);
        value["options"] = json!(field.options);
    }
    value
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
