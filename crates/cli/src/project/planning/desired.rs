use super::{
    Change, cycle,
    facts::{self, Facts},
    model::Level,
};
use concord_core::Result;
use serde_json::{Value, json};

pub(in crate::project) fn result(state: &Value, change: &Change) -> Result<Option<Value>> {
    let facts = Facts { state };
    match change {
        Change::Priority { field, item, level } => {
            facts.option(field, &Level::Low)?;
            let expected = level
                .as_ref()
                .map(|level| facts.option(field, level))
                .transpose()?;
            choice(facts.item(item)?, field, "optionId", expected)
        }
        Change::Assign {
            field,
            item,
            period,
        } => {
            facts.field(field, "ITERATION")?;
            if let Some(period) = period {
                facts.period(field, period)?;
            }
            choice(facts.item(item)?, field, "iterationId", period.as_deref())
        }
        Change::Cycle { .. } => definition(state, change),
        Change::Order { .. } => order(state, change),
    }
}

fn definition(state: &Value, change: &Change) -> Result<Option<Value>> {
    let Change::Cycle { field, name, .. } = change else {
        unreachable!("cycle definition");
    };
    let found = match field {
        Some(field) => vec![Facts { state }.field(field, "ITERATION")?],
        None => state["fields"]
            .as_array()
            .expect("qualified fields")
            .iter()
            .filter(|field| {
                field["name"] == *name
                    && field["authority"] == "project"
                    && field["dataType"] == "ITERATION"
            })
            .collect(),
    };
    let [field] = found.as_slice() else {
        return Ok(None);
    };
    Ok(cycle::matches(field, change)?.then(|| (*field).clone()))
}

fn order(state: &Value, change: &Change) -> Result<Option<Value>> {
    let Change::Order {
        field,
        period,
        item,
        after,
    } = change
    else {
        unreachable!("cycle order");
    };
    Facts { state }.period(field, period)?;
    let mut ordered = Vec::new();
    for candidate in state["items"].as_array().expect("qualified items") {
        if let Some(cell) = facts::cell(candidate, field)?
            && cell["iterationId"] == *period
        {
            ordered.push(candidate["id"].clone());
        }
    }
    let position = ordered.iter().position(|id| *id == *item);
    let expected = match after {
        Some(after) => ordered
            .iter()
            .position(|id| *id == *after)
            .map(|index| index + 1),
        None => Some(0),
    };
    Ok((position.is_some() && position == expected)
        .then(|| json!({"field":field,"period":period,"ordered_items":ordered})))
}

fn choice(item: &Value, field: &str, key: &str, expected: Option<&str>) -> Result<Option<Value>> {
    let actual = facts::selected(item, field, key)?;
    Ok((actual==expected).then(||json!({"item":item["id"],"issue":item["content"]["id"],"field":field,"selection":actual,"value":facts::cell(item,field).expect("validated native value")})))
}
