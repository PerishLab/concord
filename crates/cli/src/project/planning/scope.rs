use super::super::{fault, field};
use super::{
    Change, cycle,
    facts::{self, Facts},
    model::Level,
};
use concord_core::Result;
use serde_json::Value;

pub(in crate::project) fn qualify(state: &Value, change: &Change) -> Result<()> {
    let facts = Facts { state };
    if state["workflows"]
        .as_array()
        .expect("native workflows")
        .iter()
        .any(|workflow| workflow["enabled"] != false)
    {
        return Err(fault(
            "automation",
            "planning changes require explicitly qualified disabled native workflows; external automation remains unknown",
        ));
    }
    match change {
        Change::Priority { field, item, .. } => {
            facts.option(field, &Level::Low)?;
            facts.item(item)?;
            facts::selected(facts.item(item)?, field, "optionId")?;
        }
        Change::Cycle { .. } => cycle::qualify(state, change)?,
        Change::Assign {
            field,
            item,
            period,
        } => {
            facts.field(field, "ITERATION")?;
            facts::selected(facts.item(item)?, field, "iterationId")?;
            if let Some(period) = period {
                facts.period(field, period)?;
            }
        }
        Change::Order { .. } => order(&facts, change)?,
    }
    field::text(&summary(state, change), 48000)?;
    Ok(())
}

fn assigned(item: &Value, field: &str, period: &str) -> Result<()> {
    if facts::selected(item, field, "iterationId")? != Some(period) {
        return Err(fault(
            "period",
            "ordering anchors must belong to the exact same native cycle",
        ));
    }
    Ok(())
}

pub(in crate::project) fn summary(state: &Value, change: &Change) -> String {
    match change {
        Change::Priority { field, item, level } => format!(
            "Priority for {} through native field {field}: {}.",
            label(state, item),
            level
                .as_ref()
                .map(|level| serde_json::to_value(level)
                    .expect("level")
                    .as_str()
                    .expect("level name")
                    .to_owned())
                .unwrap_or_else(|| "unreviewed".into())
        ),
        Change::Assign {
            field,
            item,
            period,
        } => format!(
            "Cycle choice for {} through native field {field}: {}.",
            label(state, item),
            period.as_deref().unwrap_or("unassigned")
        ),
        Change::Order {
            field,
            period,
            item,
            after,
        } => format!(
            "Within native cycle {period} of field {field}, order {} after {}.",
            label(state, item),
            after
                .as_ref()
                .map(|item| label(state, item))
                .unwrap_or_else(|| "the beginning of this cycle".into())
        ),
        Change::Cycle {
            field,
            name,
            start,
            duration,
            periods,
        } => {
            let id = field
                .as_deref()
                .or_else(|| {
                    state["fields"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|field| field["name"] == *name)
                        .and_then(|field| field["id"].as_str())
                })
                .unwrap_or("new");
            let dates = periods
                .iter()
                .map(|period| {
                    let end = super::date::Date::parse(&period.start)
                        .and_then(|date| date.end(period.duration))
                        .map(super::date::Date::text)
                        .unwrap_or_else(|_| "unreadable".into());
                    format!(
                        "{} {}: [{}, {})",
                        period.id.as_deref().unwrap_or("new"),
                        period.title,
                        period.start,
                        end
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            format!(
                "Configure native cycle field {id} ({name}); first local calendar date {start}, default duration {duration} days. Explicit periods: {dates}. Calendar definitions do not prove elapsed work."
            )
        }
    }
}

fn label(state: &Value, id: &str) -> String {
    state["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["id"] == id)
        .and_then(|item| item["content"]["url"].as_str())
        .unwrap_or(id)
        .to_string()
}

fn order(facts: &Facts<'_>, change: &Change) -> Result<()> {
    let Change::Order {
        field,
        period,
        item,
        after,
    } = change
    else {
        unreachable!("cycle order");
    };
    facts.period(field, period)?;
    assigned(facts.item(item)?, field, period)?;
    if let Some(after) = after {
        if after == item {
            return Err(fault("order", "an item cannot be ordered after itself"));
        }
        assigned(facts.item(after)?, field, period)?;
    }
    Ok(())
}
