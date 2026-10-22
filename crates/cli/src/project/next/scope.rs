use super::super::{
    fault,
    planning::{self, Date},
};
use concord_core::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) struct Scope<'a> {
    pub cycle: &'a str,
    pub priority: &'a str,
    pub period: &'a Value,
    options: &'a [Value],
}

impl<'a> Scope<'a> {
    pub fn find(fields: &'a Value, day: i64) -> Result<Option<Self>> {
        let fields = fields
            .as_array()
            .ok_or_else(|| fault("schema", "planning fields are unreadable"))?;
        let owned = fields
            .iter()
            .filter(|field| field["authority"] == "project")
            .collect::<Vec<_>>();
        let cycles = owned
            .iter()
            .filter(|field| field["dataType"] == "ITERATION")
            .collect::<Vec<_>>();
        let priorities = owned
            .iter()
            .filter(|field| priority(field))
            .collect::<Vec<_>>();
        if cycles.len() != 1 || priorities.len() != 1 {
            return Ok(None);
        }
        let field = cycles[0];
        let mut active = Vec::new();
        let mut ids = BTreeSet::new();
        for period in planning::periods(field)? {
            let id = text(period, "id")?;
            if !ids.insert(id) {
                return Err(fault("schema", "native cycle identity is repeated"));
            }
            let start = Date::parse(text(period, "startDate")?)?;
            let duration = period["duration"]
                .as_u64()
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| fault("schema", "native cycle duration is unreadable"))?;
            let end = start.end(duration)?;
            if start.day() <= day && day < end.day() {
                active.push(period);
            }
        }
        if active.len() != 1 {
            return Ok(None);
        }
        let priority = priorities[0];
        Ok(Some(Self {
            cycle: text(field, "id")?,
            priority: text(priority, "id")?,
            period: active[0],
            options: priority["options"].as_array().expect("qualified options"),
        }))
    }

    pub fn priority(&self, item: &Value) -> Result<Value> {
        let Some(id) = planning::selected(item, self.priority, "optionId")? else {
            return Ok(Value::Null);
        };
        let option = self
            .options
            .iter()
            .find(|option| option["id"] == id)
            .ok_or_else(|| {
                fault(
                    "schema",
                    "item priority is outside the Project-owned options",
                )
            })?;
        Ok(json!({"id":id,"name":option["name"]}))
    }
}

fn priority(field: &Value) -> bool {
    if field["dataType"] != "SINGLE_SELECT" {
        return false;
    }
    let Some(options) = field["options"].as_array() else {
        return false;
    };
    let names = options
        .iter()
        .filter_map(|option| option["name"].as_str())
        .collect::<BTreeSet<_>>();
    let ids = options
        .iter()
        .filter_map(|option| option["id"].as_str().filter(|id| !id.is_empty()))
        .collect::<BTreeSet<_>>();
    options.len() == 4
        && ids.len() == 4
        && names == BTreeSet::from(["Urgent", "High", "Medium", "Low"])
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| fault("schema", format!("native planning {key} is unreadable")))
}
