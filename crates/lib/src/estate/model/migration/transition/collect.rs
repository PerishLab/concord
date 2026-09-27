use super::super::super::super::{Agreement, Estate, Life};
use super::{Counts, Dependency, Evidence, Inventory, Member, SCHEMA, SOURCE, TARGET, Task};
use crate::{Error, Result, git};
use std::collections::BTreeMap;

pub(super) async fn read(
    estate: &Estate,
    agreement: Agreement,
    database: Evidence,
    sudo: Evidence,
) -> Result<Inventory> {
    let domains = estate.realms().await?;
    let repositories = estate.repositories(None, true).await?;
    let graph = estate.graph(true).await?;
    let identities = graph
        .nodes
        .iter()
        .map(|task| (task.key, task.identity()))
        .collect::<BTreeMap<_, _>>();
    let mut tasks = Vec::with_capacity(graph.nodes.len());
    for task in &graph.nodes {
        let identity = task.identity();
        let current = estate.current(&identity).await?;
        let phases = estate.phases(&identity).await?;
        tasks.push(Task {
            key: task.key,
            identity,
            life: task.life,
            revision: task.revision,
            facts: current.facts.len(),
            phases: phases.len(),
            phase_entries: phases.iter().map(|phase| phase.entries.len()).sum(),
            reference: current.reference,
        });
    }
    let dependencies = graph
        .edges
        .iter()
        .map(|edge| {
            Ok(Dependency {
                key: edge.key,
                source: identities
                    .get(&edge.source)
                    .cloned()
                    .ok_or_else(|| dependency(edge.key, edge.source))?,
                target: identities
                    .get(&edge.target)
                    .cloned()
                    .ok_or_else(|| dependency(edge.key, edge.target))?,
                weight: edge.weight,
                origin: edge.origin,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut members = Vec::new();
    for member in estate.worktrees().await? {
        let task = graph
            .nodes
            .iter()
            .find(|task| task.identity() == member.task)
            .ok_or_else(|| {
                Error::typed(
                    "concord.transition.member",
                    format!(
                        "Member has an unknown Task: {}/{}",
                        member.task, member.name
                    ),
                )
            })?;
        let path = estate.path(&task.domain, &task.name, &member.name);
        let source_path = estate.source(&member)?.canonicalize()?;
        let path = path.canonicalize()?;
        let source_identity = git::at(&source_path).identity()?;
        if git::at(&path).identity()? != source_identity
            || !git::at(&source_path).registered(&path)?
            || git::at(&path).branch()? != member.branch
        {
            return Err(Error::typed(
                "concord.transition.member",
                format!(
                    "Member seat identity disagrees: {}/{}",
                    member.task, member.name
                ),
            ));
        }
        members.push(Member {
            key: member.key,
            task: member.task.clone(),
            name: member.name.clone(),
            source: member.source.clone(),
            source_path,
            source_identity,
            branch: member.branch.clone(),
            path,
            head: git::at(&estate.path(&task.domain, &task.name, &member.name)).head()?,
            claims: member.claims.clone(),
            boundary: member.proof.clone(),
            change: estate.forge(task.key, Some(member.key)).await?,
        });
    }
    members.sort_by_key(|member| (member.task.clone(), member.name.clone()));
    let mut artifacts = Vec::new();
    for task in &graph.nodes {
        let identity = task.identity();
        for artifact in estate.artifacts(&identity).await? {
            artifacts.push(super::source::artifact(
                &artifact.task,
                &artifact.name,
                &artifact.path,
            )?);
        }
    }
    artifacts.sort_by_key(|artifact| (artifact.task.clone(), artifact.name.clone()));
    let counts = Counts {
        domains: domains.len(),
        active_tasks: tasks
            .iter()
            .filter(|task| task.life == Life::Active)
            .count(),
        retired_tasks: tasks
            .iter()
            .filter(|task| task.life == Life::Retired)
            .count(),
        facts: tasks.iter().map(|task| task.facts).sum(),
        phases: tasks.iter().map(|task| task.phases).sum(),
        phase_entries: tasks.iter().map(|task| task.phase_entries).sum(),
        dependencies: dependencies.len(),
        members: members.len(),
        claims: members.iter().map(|member| member.claims.len()).sum(),
        boundaries: members
            .iter()
            .filter(|member| member.boundary.is_some())
            .count(),
        artifacts: artifacts.len(),
        filesystem_seats: members.len() + artifacts.len(),
    };
    Ok(Inventory {
        schema: SCHEMA.to_string(),
        source: SOURCE.to_string(),
        target: TARGET.to_string(),
        fingerprint: String::new(),
        database,
        sudo,
        graph_revision: graph.revision,
        domains,
        repositories,
        tasks,
        dependencies,
        members,
        artifacts,
        counts,
        observations: agreement.observations,
    })
}

fn dependency(edge: i64, endpoint: i64) -> Error {
    Error::typed(
        "concord.transition.dependency",
        format!("Dependency {edge} references unknown Task {endpoint}"),
    )
}
