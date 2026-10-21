use super::super::manage::{Action, Intent, Step};
use super::{Change, cycle, facts::Facts, scope};
use concord_core::Result;
use serde_json::{Value, json};

pub(in crate::project) fn build(state: &Value, intent: &Intent) -> Result<Vec<Step>> {
    let Action::Planning { change } = &intent.action else {
        unreachable!("planning intent");
    };
    scope::qualify(state, change)?;
    if super::desired::result(state, change)?.is_some() {
        return Ok(Vec::new());
    }
    let (mutation, input) = match change {
        Change::Priority { .. } | Change::Assign { .. } => value(state, change)?,
        Change::Order { item, after, .. } => (
            "updateProjectV2ItemPosition",
            json!({"projectId":state["project"]["id"],"itemId":item,"afterId":after}),
        ),
        Change::Cycle { .. } => definition(state, change),
    };
    Ok(vec![Step {
        mutation,
        input,
        check: intent.clone(),
    }])
}

fn value(state: &Value, change: &Change) -> Result<(&'static str, Value)> {
    let (item, field, value) = match change {
        Change::Priority { field, item, level } => {
            let option = level
                .as_ref()
                .map(|level| Facts { state }.option(field, level))
                .transpose()?;
            (
                item,
                field,
                option.map(|id| json!({"singleSelectOptionId":id})),
            )
        }
        Change::Assign {
            field,
            item,
            period,
        } => (
            item,
            field,
            period.as_ref().map(|period| json!({"iterationId":period})),
        ),
        _ => unreachable!("item value change"),
    };
    let mut input = json!({"projectId":state["project"]["id"],"itemId":item,"fieldId":field});
    Ok(match value {
        Some(value) => {
            input["value"] = value;
            ("updateProjectV2ItemFieldValue", input)
        }
        None => ("clearProjectV2ItemFieldValue", input),
    })
}

fn definition(state: &Value, change: &Change) -> (&'static str, Value) {
    let Change::Cycle { field, name, .. } = change else {
        unreachable!("cycle definition");
    };
    let configuration = cycle::input(change);
    match field {
        Some(field) => (
            "updateProjectV2Field",
            json!({"fieldId":field,"name":name,"iterationConfiguration":configuration}),
        ),
        None => (
            "createProjectV2Field",
            json!({"projectId":state["project"]["id"],"name":name,"dataType":"ITERATION","iterationConfiguration":configuration}),
        ),
    }
}
