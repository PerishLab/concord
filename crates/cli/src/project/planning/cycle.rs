use super::super::{fault, field};
use super::{
    date::Date,
    facts::{self, Facts},
    model::Period,
};
use concord_core::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn qualify(state: &Value, change: &super::Change) -> Result<()> {
    let super::Change::Cycle {
        field: id,
        name,
        start,
        duration,
        periods,
    } = change
    else {
        unreachable!("cycle definition");
    };
    field::text(name, 255)?;
    let first = Date::parse(start)?;
    first.end(*duration)?;
    if periods.is_empty() || periods.len() > 100 {
        return Err(fault(
            "bounds",
            "cycle configuration requires 1..100 explicit periods",
        ));
    }
    let mut intervals = Vec::new();
    let mut identities = BTreeSet::new();
    for period in periods {
        field::text(&period.title, 255)?;
        let start = Date::parse(&period.start)?;
        intervals.push((start, start.end(period.duration)?));
        if let Some(id) = &period.id
            && (id.is_empty() || !identities.insert(id))
        {
            return Err(fault("identity", "period identity is empty or repeated"));
        }
    }
    intervals.sort();
    if intervals[0].0 != first {
        return Err(fault(
            "date",
            "field start must equal the first explicitly supplied calendar period",
        ));
    }
    if intervals.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(fault("date", "native cycle periods overlap"));
    }
    match id {
        Some(id) => preserve(Facts { state }.field(id, "ITERATION")?, periods)?,
        None => {
            if periods.iter().any(|period| period.id.is_some()) {
                return Err(fault(
                    "identity",
                    "a new cycle field cannot import existing period identities",
                ));
            }
        }
    }
    let conflict = state["fields"]
        .as_array()
        .expect("qualified fields")
        .iter()
        .any(|field| field["name"] == *name && id.as_ref().is_none_or(|id| field["id"] != *id));
    if conflict {
        return Err(fault(
            "ambiguous",
            "cycle field name already exists; select the exact owned iteration field",
        ));
    }
    Ok(())
}

pub(super) fn input(change: &super::Change) -> Value {
    let super::Change::Cycle {
        start,
        duration,
        periods,
        ..
    } = change
    else {
        unreachable!("cycle definition");
    };
    let periods=periods.iter().map(|period|json!({"id":period.id,"title":period.title,"startDate":period.start,"duration":period.duration})).collect::<Vec<_>>();
    json!({"startDate":start,"duration":duration,"iterations":periods})
}

pub(super) fn matches(field: &Value, change: &super::Change) -> Result<bool> {
    let super::Change::Cycle {
        name,
        duration,
        periods,
        ..
    } = change
    else {
        unreachable!("cycle definition");
    };
    if field["name"] != *name || field["configuration"]["duration"] != *duration {
        return Ok(false);
    }
    let actual = facts::periods(field)?;
    Ok(actual.len() == periods.len()
        && periods
            .iter()
            .all(|period| actual.iter().any(|actual| same(actual, period))))
}

fn same(actual: &Value, period: &Period) -> bool {
    let identity = period.id.as_ref().is_none_or(|id| actual["id"] == *id);
    let metadata = actual["title"] == period.title
        && actual["startDate"] == period.start
        && actual["duration"] == period.duration;
    identity && metadata
}

fn preserve(field: &Value, periods: &[Period]) -> Result<()> {
    let current = facts::periods(field)?;
    for prior in &current {
        let id = prior["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| fault("schema", "existing period identity is unreadable"))?;
        if !periods
            .iter()
            .any(|period| period.id.as_deref() == Some(id))
        {
            return Err(fault(
                "history",
                "cycle update must preserve every active, future and completed period identity",
            ));
        }
    }
    for period in periods {
        if let Some(id) = &period.id
            && !current.iter().any(|prior| prior["id"] == *id)
        {
            return Err(fault(
                "period",
                "period identity belongs to another field or is unknown",
            ));
        }
    }
    Ok(())
}
