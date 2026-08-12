use super::super::super::{Estate, Origin, Weight};
use super::super::load;
use super::super::scan::Work;
use super::member;
use super::{exact, mismatch};
use crate::{Graph, Result, Worktree};
use keel::Row;
use std::collections::BTreeMap;

pub(super) struct Context<'a> {
    pub estate: &'a Estate,
    pub graph: &'a Graph,
    pub nodes: &'a BTreeMap<String, i64>,
    pub members: &'a [Worktree],
    pub additions: &'a [Row],
}

pub(super) async fn check(context: &Context<'_>, domain: &str, work: &Work) -> Result<()> {
    let identity = format!("{domain}/{}", work.task.name);
    if !context.nodes.contains_key(&identity) {
        return Err(mismatch(format!("missing Task {identity}")));
    }
    current(context.estate, &identity, work).await?;
    phases(context.estate, &identity, work).await?;
    member::check(context.members, context.additions, &identity, work)?;
    dependencies(context, domain, &identity, work).await
}

async fn current(estate: &Estate, identity: &str, work: &Work) -> Result<()> {
    let mut wanted = load::current(work);
    let mut found = estate.current(identity).await?.facts;
    for fact in &mut found {
        fact.key = None;
    }
    wanted.sort_by_key(|fact| (fact.role, fact.rank.unwrap_or(0)));
    found.sort_by_key(|fact| (fact.role, fact.rank.unwrap_or(0)));
    if found == wanted {
        return Ok(());
    }
    Err(mismatch(format!("current facts differ for {identity}")))
}

async fn phases(estate: &Estate, identity: &str, work: &Work) -> Result<()> {
    let phases = estate.phases(identity).await?;
    exact("Task Phase", work.phases.len(), phases.len())?;
    for (index, (wanted, found)) in work.phases.iter().zip(phases).enumerate() {
        if found.number != index as i64 + 1 {
            return Err(mismatch(format!("Phase serial differs for {identity}")));
        }
        let mut entries = found.entries;
        for entry in &mut entries {
            entry.key = None;
        }
        if entries != *wanted {
            return Err(mismatch(format!("Phase facts differ for {identity}")));
        }
    }
    Ok(())
}

async fn dependencies(
    context: &Context<'_>,
    domain: &str,
    identity: &str,
    work: &Work,
) -> Result<()> {
    let source = context.nodes[identity];
    for held in &work.task.todo {
        let target = format!("{domain}/{held}");
        let key = context.nodes[&target];
        let edge = context
            .graph
            .edges
            .iter()
            .find(|edge| edge.source == source && edge.target == key)
            .ok_or_else(|| mismatch(format!("missing dependency {identity} -> {target}")))?;
        if (edge.weight, edge.origin) != (Weight::Unknown, Origin::Legacy) {
            return Err(mismatch(format!(
                "legacy dependency attributes differ: {identity} -> {target}"
            )));
        }
        let reached = context
            .estate
            .reach(identity)
            .await?
            .iter()
            .any(|node| node.key == key);
        if !reached {
            return Err(mismatch(format!("closure misses {identity} -> {target}")));
        }
    }
    Ok(())
}
