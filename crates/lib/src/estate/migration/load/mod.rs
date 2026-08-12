mod domain;
mod extra;
mod member;
mod task;
mod text;

use super::super::{Estate, Role, fault};
use super::scan::{Source, Work};
use crate::{Fact, Result};
use keel::adapt::db::Sqlite;
use keel::{Ends, Tx};
use std::collections::BTreeMap;

pub(super) use text::{entries, facts};

pub(super) async fn write(estate: &Estate, source: &Source) -> Result<()> {
    let space = estate
        .core
        .live("Space")
        .await
        .map_err(fault)?
        .into_iter()
        .next()
        .expect("staged estate has seeded Space")
        .key();
    let space = space.to_string();
    let loaded = estate
        .core
        .batch(async |tx| {
            let mut tasks = BTreeMap::new();
            for realm in &source.realms {
                domain::seed(tx, realm, &space, &mut tasks).await?;
            }
            dependencies(tx, source, &tasks).await?;
            Ok(())
        })
        .await;
    match loaded {
        Ok(()) => Ok(()),
        Err(error) if error.to_string().contains("cycle") => Err(crate::Error::typed(
            "concord.migration.todo_cycle",
            "legacy todo graph contains a cycle",
        )),
        Err(error) => Err(fault(error)),
    }
}

async fn dependencies(
    tx: &mut Tx<'_, Sqlite>,
    source: &Source,
    tasks: &BTreeMap<String, i64>,
) -> std::result::Result<(), keel::adapt::Error> {
    for realm in &source.realms {
        for work in &realm.works {
            let left = tasks[&format!("{}/{}", realm.name, work.task.name)];
            for todo in &work.task.todo {
                let right = tasks[&format!("{}/{}", realm.name, todo)];
                tx.tie(
                    "Task",
                    "depends",
                    Ends { left, right },
                    &[("weight", "unknown"), ("origin", "legacy-todo")],
                )
                .await?;
            }
        }
    }
    Ok(())
}

pub(super) fn current(work: &Work) -> Vec<Fact> {
    let mut held = work.facts.clone();
    let mut rank = held
        .iter()
        .filter(|fact| fact.role == Role::Addition)
        .filter_map(|fact| fact.rank)
        .max()
        .unwrap_or(0);
    for (name, value) in &work.task.extra {
        rank += 1;
        held.push(Fact {
            key: None,
            role: Role::Addition,
            rank: Some(rank),
            title: Some(name.clone()),
            body: value.to_string(),
            origin: Some("legacy-registry".to_string()),
        });
    }
    held
}
