use super::super::manage::{Action, Cell, Intent, Step};
use super::super::{fault, field, planning};
use concord_core::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(in crate::project) fn qualify(state: &Value, intent: &Intent) -> Result<()> {
    let Action::Value {
        field: id,
        item,
        selection,
    } = &intent.action
    else {
        unreachable!("local value intent")
    };
    let field = field::owned(state, id)?;
    let current = planning::Facts { state }.item(item)?;
    if current["content"]["state"] != "OPEN" {
        return Err(fault(
            "scope",
            "closed Issue history cannot use local relation maintenance",
        ));
    }
    basis(field, planning::cell(current, id)?)?;
    if state["workflows"]
        .as_array()
        .expect("qualified workflows")
        .iter()
        .any(|workflow| workflow["enabled"] != false)
    {
        return Err(fault(
            "automation",
            "local value maintenance requires disabled native workflows; external automation remains unknown",
        ));
    }
    protect(field)?;
    match (field["dataType"].as_str(), selection) {
        (Some("TEXT"), Cell::Text { text }) => field::text(text, 4096)?,
        (Some("SINGLE_SELECT"), Cell::Select { option }) => {
            let ids = options(field)?;
            if !ids.contains(option.as_str()) {
                return Err(fault(
                    "identity",
                    "option is outside this exact native field",
                ));
            }
        }
        (Some("TEXT" | "SINGLE_SELECT"), Cell::Clear) => {}
        _ => {
            return Err(fault(
                "type",
                "local values support only matching TEXT or SINGLE_SELECT and explicit clear",
            ));
        }
    }
    Ok(())
}

fn basis(field: &Value, previous: Option<&Value>) -> Result<()> {
    let Some(value) = previous else {
        return Ok(());
    };
    match field["dataType"].as_str() {
        Some("TEXT")
            if value["__typename"] == "ProjectV2ItemFieldTextValue"
                && value["text"].is_string() =>
        {
            Ok(())
        }
        Some("SINGLE_SELECT") if value["__typename"] == "ProjectV2ItemFieldSingleSelectValue" => {
            let id = value["optionId"]
                .as_str()
                .ok_or_else(|| fault("schema", "prior option identity is unreadable"))?;
            if options(field)?.contains(id) {
                Ok(())
            } else {
                Err(fault("schema", "prior option is outside its native field"))
            }
        }
        _ => Err(fault(
            "schema",
            "prior local value disagrees with its native field type",
        )),
    }
}

fn protect(field: &Value) -> Result<()> {
    let name = field["name"].as_str().unwrap_or("");
    if ["Status", "Priority", "Cycle", "Phase", "Planning"]
        .iter()
        .any(|reserved| name.eq_ignore_ascii_case(reserved))
    {
        return Err(fault(
            "scope",
            "reserved lifecycle and planning fields cannot use the local value operation",
        ));
    }
    if field["dataType"] != "SINGLE_SELECT" {
        return Ok(());
    }
    let options = field["options"]
        .as_array()
        .ok_or_else(|| fault("schema", "select options are unreadable"))?;
    let names = options
        .iter()
        .filter_map(|option| option["name"].as_str())
        .collect::<BTreeSet<_>>();
    if names == BTreeSet::from(["Urgent", "High", "Medium", "Low"])
        || names == BTreeSet::from(["Todo", "In Progress", "Done"])
    {
        return Err(fault(
            "scope",
            "canonical priority or status schema requires its dedicated operation",
        ));
    }
    let _ = self::options(field)?;
    Ok(())
}

fn options(field: &Value) -> Result<BTreeSet<&str>> {
    let values = field["options"]
        .as_array()
        .ok_or_else(|| fault("schema", "select options are unreadable"))?;
    let mut ids = BTreeSet::new();
    for value in values {
        if value["name"].as_str().is_none_or(str::is_empty) {
            return Err(fault("schema", "option name is unreadable"));
        }
        let id = value["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| fault("schema", "option identity is unreadable"))?;
        if !ids.insert(id) {
            return Err(fault("schema", "option identity is repeated"));
        }
    }
    Ok(ids)
}

pub(in crate::project) fn result(state: &Value, intent: &Intent) -> Result<Option<Value>> {
    qualify(state, intent)?;
    let Action::Value {
        field,
        item,
        selection,
    } = &intent.action
    else {
        unreachable!("local value result")
    };
    let current = planning::cell(planning::Facts { state }.item(item)?, field)?;
    let matches = match (current, selection) {
        (None, Cell::Clear) => true,
        (Some(value), Cell::Text { text }) => {
            value["__typename"] == "ProjectV2ItemFieldTextValue" && value["text"] == *text
        }
        (Some(value), Cell::Select { option }) => {
            value["__typename"] == "ProjectV2ItemFieldSingleSelectValue"
                && value["optionId"] == *option
        }
        _ => false,
    };
    Ok(matches.then(|| json!({"item":item,"field":field,"value":current})))
}

pub(in crate::project) fn steps(state: &Value, intent: &Intent) -> Result<Vec<Step>> {
    if result(state, intent)?.is_some() {
        return Ok(Vec::new());
    }
    let Action::Value {
        field,
        item,
        selection,
    } = &intent.action
    else {
        unreachable!("local value steps")
    };
    let mut input = json!({"projectId":state["project"]["id"],"fieldId":field,"itemId":item});
    let mutation = match selection {
        Cell::Text { text } => {
            input["value"] = json!({"text":text});
            "updateProjectV2ItemFieldValue"
        }
        Cell::Select { option } => {
            input["value"] = json!({"singleSelectOptionId":option});
            "updateProjectV2ItemFieldValue"
        }
        Cell::Clear => "clearProjectV2ItemFieldValue",
    };
    Ok(vec![Step {
        mutation,
        input,
        check: intent.clone(),
    }])
}

pub(in crate::project) fn selection(item: &str) -> String {
    planning::selection(item).replace(
        "ProjectV2ItemFieldTextValue{field",
        "ProjectV2ItemFieldTextValue{text field",
    )
}
