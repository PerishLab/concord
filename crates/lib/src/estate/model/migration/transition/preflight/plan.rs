use super::super::Inventory;
use super::{Disposition, DispositionPlan, IssueDestination, Observation, TaskDisposition};
use crate::{Error, Life, Result};
use std::collections::{BTreeMap, BTreeSet};

pub struct Checked {
    pub tasks: BTreeMap<String, TaskDisposition>,
    pub issues: Vec<Observation>,
}

pub fn check(
    plan: &DispositionPlan,
    observations: &[Observation],
    inventory: &Inventory,
) -> Result<Checked> {
    let tasks = tasks(&plan.tasks)?;
    let expected = inventory
        .tasks
        .iter()
        .map(|task| (task.identity.as_str(), task))
        .collect::<BTreeMap<_, _>>();
    for task in tasks.keys() {
        if !expected.contains_key(task.as_str()) {
            return Err(Error::typed(
                "concord.transition.disposition_extraneous",
                format!("disposition names an unknown Task: {task}"),
            ));
        }
    }
    for (identity, source) in &expected {
        let disposition = tasks.get(*identity).ok_or_else(|| {
            Error::typed(
                "concord.transition.disposition_missing",
                format!("Task has no disposition: {identity}"),
            )
        })?;
        if disposition.revision != source.revision {
            return Err(Error::typed(
                "concord.transition.task_drift",
                format!("Task revision changed: {identity}"),
            ));
        }
        if source.life == Life::Retired
            && !matches!(disposition.disposition, Disposition::ArchiveOnly)
        {
            return Err(Error::typed(
                "concord.transition.retired_destination",
                format!("retired Task may only enter private archive lineage: {identity}"),
            ));
        }
    }
    let issues = tasks
        .values()
        .filter_map(|entry| match &entry.disposition {
            Disposition::Issue { issue } => Some(issue),
            Disposition::ArchiveOnly => None,
        })
        .collect::<Vec<_>>();
    nodes(&issues)?;
    let issues = observe(&issues, observations)?;
    Ok(Checked { tasks, issues })
}

fn tasks(dispositions: &[TaskDisposition]) -> Result<BTreeMap<String, TaskDisposition>> {
    let mut held = BTreeMap::new();
    for disposition in dispositions {
        if let Some(prior) = held.insert(disposition.task.clone(), disposition.clone()) {
            let code = if prior == *disposition {
                "concord.transition.disposition_duplicate"
            } else {
                "concord.transition.disposition_ambiguous"
            };
            return Err(Error::typed(
                code,
                format!("Task has multiple dispositions: {}", disposition.task),
            ));
        }
    }
    Ok(held)
}

fn nodes(issues: &[&IssueDestination]) -> Result<()> {
    let mut coordinates = BTreeMap::new();
    for issue in issues {
        if issue.node.trim().is_empty() {
            return Err(Error::typed(
                "concord.transition.issue_node",
                "Issue destination has no stable node identity",
            ));
        }
        let identity = issue.coordinate.identity();
        if let Some(node) = coordinates.insert(identity.clone(), issue.node.as_str())
            && node != issue.node
        {
            return Err(Error::typed(
                "concord.transition.issue_ambiguous",
                format!("Issue coordinate names multiple nodes: {identity}"),
            ));
        }
    }
    Ok(())
}

fn observe(declared: &[&IssueDestination], found: &[Observation]) -> Result<Vec<Observation>> {
    let mut live = BTreeMap::new();
    for observation in found {
        let identity = observation.coordinate.identity();
        if live.insert(identity.clone(), observation).is_some() {
            return Err(Error::typed(
                "concord.transition.provider_duplicate",
                format!("provider returned duplicate Issue observation: {identity}"),
            ));
        }
    }
    let expected = declared
        .iter()
        .map(|issue| issue.coordinate.identity())
        .collect::<BTreeSet<_>>();
    if live.keys().any(|identity| !expected.contains(identity)) {
        return Err(Error::typed(
            "concord.transition.provider_extraneous",
            "provider returned an Issue outside the disposition plan",
        ));
    }
    let mut checked = Vec::new();
    for issue in declared {
        let identity = issue.coordinate.identity();
        let observed = live.get(&identity).ok_or_else(|| {
            Error::typed(
                "concord.transition.provider_missing",
                format!("Issue was not observed: {identity}"),
            )
        })?;
        if observed.node != issue.node {
            return Err(Error::typed(
                "concord.transition.provider_node_drift",
                format!("stable Issue node changed at {identity}"),
            ));
        }
        checked.push((*observed).clone());
    }
    checked.sort_by_key(|issue| issue.coordinate.identity());
    checked.dedup();
    Ok(checked)
}
