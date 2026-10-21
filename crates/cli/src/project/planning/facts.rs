use super::super::{fault, field};
use super::model::Level;
use concord_core::Result;
use serde_json::Value;

pub(super) struct Facts<'a> {
    pub state: &'a Value,
}

impl<'a> Facts<'a> {
    pub fn field(&self, id: &str, kind: &str) -> Result<&'a Value> {
        let field = field::owned(self.state, id)?;
        if field["dataType"] != kind {
            return Err(fault("type", "planning field has the wrong native type"));
        }
        Ok(field)
    }

    pub fn item(&self, id: &str) -> Result<&'a Value> {
        let item = self.state["items"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["id"] == id))
            .ok_or_else(|| {
                fault(
                    "identity",
                    "planning item is outside this qualified Project",
                )
            })?;
        let issue = &item["content"];
        if item["project"]["id"] != self.state["project"]["id"] || item["isArchived"] != false {
            return Err(fault("scope", "planning item is foreign or archived"));
        }
        if issue["__typename"] != "Issue"
            || issue["repository"]["owner"]["id"] != self.state["project"]["owner"]["id"]
        {
            return Err(fault(
                "identity",
                "planning choice requires an original Issue in this organization scope",
            ));
        }
        if !matches!(
            issue["issueType"]["name"].as_str(),
            Some("Feature" | "Bug" | "Task")
        ) {
            return Err(fault(
                "custody",
                "Auto and unknown Issue types are read-only observations",
            ));
        }
        Ok(item)
    }

    pub fn period(&self, field: &str, id: &str) -> Result<&'a Value> {
        let field = self.field(field, "ITERATION")?;
        let period = periods(field)?
            .into_iter()
            .find(|period| period["id"] == id)
            .ok_or_else(|| {
                fault(
                    "period",
                    "cycle identity is not in this exact Project-owned iteration field",
                )
            })?;
        let start = period["startDate"]
            .as_str()
            .ok_or_else(|| fault("schema", "native period date is unreadable"))?;
        let duration = period["duration"]
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(|| fault("schema", "native period duration is unreadable"))?;
        super::date::Date::parse(start)?.end(duration)?;
        Ok(period)
    }

    pub fn option(&self, field: &str, level: &Level) -> Result<&'a str> {
        let field = self.field(field, "SINGLE_SELECT")?;
        let options = field["options"]
            .as_array()
            .ok_or_else(|| fault("schema", "priority option identities are unreadable"))?;
        let expected = ["Urgent", "High", "Medium", "Low"];
        let mut names = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        for option in options {
            let name = option["name"]
                .as_str()
                .ok_or_else(|| fault("schema", "priority option name is unreadable"))?;
            let id = option["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| fault("schema", "priority option ID is unreadable"))?;
            if !expected.contains(&name) || !names.insert(name) || !ids.insert(id) {
                return Err(fault(
                    "priority",
                    "priority requires exactly four distinct native Urgent/High/Medium/Low options",
                ));
            }
        }
        if options.len() != 4 {
            return Err(fault(
                "priority",
                "priority requires Urgent, High, Medium and Low; an empty item value remains unreviewed",
            ));
        }
        let name = serde_json::to_value(level).expect("priority level");
        options
            .iter()
            .find(|option| option["name"] == name)
            .and_then(|option| option["id"].as_str())
            .ok_or_else(|| fault("priority", "selected priority option is unavailable"))
    }
}

pub(super) fn periods(field: &Value) -> Result<Vec<&Value>> {
    let configuration = &field["configuration"];
    let current = configuration["iterations"]
        .as_array()
        .ok_or_else(|| fault("schema", "native active/future cycles are unreadable"))?;
    let completed = configuration["completedIterations"]
        .as_array()
        .ok_or_else(|| fault("schema", "native completed cycles are unreadable"))?;
    Ok(current.iter().chain(completed).collect())
}

pub(super) fn cell<'a>(item: &'a Value, field: &str) -> Result<Option<&'a Value>> {
    let connection = &item["fieldValues"];
    let values = connection["nodes"]
        .as_array()
        .ok_or_else(|| fault("incomplete", "native item planning values are unreadable"))?;
    if connection["pageInfo"]["hasNextPage"] != false
        || connection["totalCount"].as_u64() != Some(values.len() as u64)
    {
        return Err(fault(
            "incomplete",
            "native item field-value coverage is incomplete",
        ));
    }
    let mut found = None;
    for value in values {
        if !value["field"]["id"].is_string() {
            return Err(fault(
                "schema",
                "an unknown native field value cannot establish an empty planning choice",
            ));
        }
        if value["field"]["id"] == field {
            if found.is_some() {
                return Err(fault(
                    "schema",
                    "native planning field identity repeated within one item",
                ));
            }
            found = Some(value);
        }
    }
    Ok(found)
}

pub(super) fn selected<'a>(item: &'a Value, field: &str, key: &str) -> Result<Option<&'a str>> {
    let Some(value) = cell(item, field)? else {
        return Ok(None);
    };
    let expected = match key {
        "optionId" => "ProjectV2ItemFieldSingleSelectValue",
        "iterationId" => "ProjectV2ItemFieldIterationValue",
        _ => unreachable!("compiled native choice"),
    };
    if value["__typename"] != expected {
        return Err(fault(
            "schema",
            "native planning value type disagrees with its owned field",
        ));
    }
    match value.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(id)) if !id.is_empty() => Ok(Some(id)),
        _ => Err(fault(
            "schema",
            "native planning selection is unreadable; it is not an empty choice",
        )),
    }
}
