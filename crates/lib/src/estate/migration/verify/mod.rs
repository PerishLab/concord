mod domain;
mod extra;
mod member;
mod task;

use super::super::{Estate, fault};
use super::scan::Source;
use crate::{Error, Result};
use std::collections::BTreeMap;

pub(super) async fn check(estate: &Estate, source: &Source) -> Result<()> {
    estate.verify().await?;
    let graph = estate.graph(false).await?;
    exact(
        "Domain",
        source.census.domain,
        estate.core.live("Domain").await.map_err(fault)?.len(),
    )?;
    domain::check(estate, source).await?;
    exact(
        "Repository",
        source.census.repository,
        estate.core.live("Repository").await.map_err(fault)?.len(),
    )?;
    exact("Task", source.census.task, graph.nodes.len())?;
    exact(
        "Reservation",
        source.census.task,
        estate.core.live("Reservation").await.map_err(fault)?.len(),
    )?;
    exact(
        "Member",
        source.census.member,
        estate.core.live("Member").await.map_err(fault)?.len(),
    )?;
    exact(
        "Claim",
        source.census.claim,
        estate.core.live("Claim").await.map_err(fault)?.len(),
    )?;
    exact(
        "Boundary",
        source.census.boundary,
        estate.core.live("Boundary").await.map_err(fault)?.len(),
    )?;
    exact("Dependency", source.census.dependency, graph.edges.len())?;
    exact(
        "Phase",
        source.census.phase,
        estate.core.live("Phase").await.map_err(fault)?.len(),
    )?;
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.identity(), node.key))
        .collect::<BTreeMap<_, _>>();
    let members = estate.worktrees().await?;
    let additions = estate.core.live("member:addition").await.map_err(fault)?;
    let context = task::Context {
        estate,
        graph: &graph,
        nodes: &nodes,
        members: &members,
        additions: &additions,
    };
    for realm in &source.realms {
        for work in &realm.works {
            task::check(&context, &realm.name, work).await?;
        }
    }
    Ok(())
}

pub(super) fn exact(label: &str, wanted: usize, found: usize) -> Result<()> {
    if wanted == found {
        return Ok(());
    }
    Err(mismatch(format!(
        "{label} count differs: expected {wanted}, found {found}"
    )))
}

pub(super) fn mismatch(message: impl Into<String>) -> Error {
    Error::typed("concord.migration.mismatch", message.into())
}
