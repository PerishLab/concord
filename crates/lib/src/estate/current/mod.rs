pub(super) mod prepare;
pub(super) mod write;

use super::World;
use super::{Current, Estate, Fact, Life, Patch, Role, fault};
use crate::{Error, Result};
use keel::Row;

impl Estate {
    #[locus::trace(with = crate::observation::view())]
    pub async fn current(&self, identity: &str) -> Result<Current> {
        let world = World::load(self).await?;
        let task = world.node(identity)?.clone();
        let facts = self.facts(task.key).await?;
        let reference = self.forge(task.key, None).await?;
        Ok(Current {
            task,
            facts,
            reference,
        })
    }

    pub async fn change(&self, patch: &Patch) -> Result<Current> {
        envelope(patch)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&patch.task)?;
        active(task.life, &patch.task)?;
        if task.revision != patch.revision {
            return Err(Error::typed(
                "concord.task.stale",
                format!(
                    "Task revision changed: expected {}, found {}",
                    patch.revision, task.revision
                ),
            ));
        }
        let current = self.facts(task.key).await?;
        let prepared = prepare::build(&current, &patch.edits)?;
        let revision = task.revision + 1;
        let next = revision.to_string();
        let root = task.key;
        let temporary = prepare::temporary(&current, &prepared)?;
        self.core
            .batch(async |tx| {
                write::apply(tx, root, &prepared, &temporary).await?;
                tx.set("Task", root, &[("revision", next.as_str())]).await?;
                Ok(())
            })
            .await
            .map_err(fault)?;
        let mut task = task.clone();
        task.revision = revision;
        Ok(Current {
            task,
            facts: self.facts(root).await?,
            reference: self.forge(root, None).await?,
        })
    }

    #[locus::trace(with = crate::observation::view())]
    pub(super) async fn facts(&self, task: i64) -> Result<Vec<Fact>> {
        let mut facts = Vec::new();
        for role in Role::ALL {
            for row in self.core.live(role.unit()).await.map_err(fault)? {
                if row.int("task") == Some(task) {
                    let fact = read(role, &row)?;
                    prepare::validate(&fact, true)?;
                    facts.push(fact);
                }
            }
        }
        facts.sort_by_key(|fact| (fact.role, fact.rank.unwrap_or(0), fact.key.unwrap_or(0)));
        Ok(facts)
    }
}

fn envelope(patch: &Patch) -> Result<()> {
    if patch.version != 1 {
        return Err(Error::typed(
            "concord.change.version",
            format!("unsupported change-set version {}", patch.version),
        ));
    }
    if patch.edits.is_empty() {
        return Err(Error::typed(
            "concord.change.empty",
            "change-set needs at least one explicit edit",
        ));
    }
    Ok(())
}

fn active(life: Life, identity: &str) -> Result<()> {
    if life == Life::Active {
        return Ok(());
    }
    Err(Error::typed(
        "concord.task.retired",
        format!("retired Task is immutable: {identity}"),
    ))
}

fn read(role: Role, row: &Row) -> Result<Fact> {
    let body = row
        .text("body")
        .filter(|body| !body.trim().is_empty())
        .ok_or_else(|| {
            Error::typed(
                "concord.change.body",
                format!("fact {} has a blank body", row.key()),
            )
        })?;
    Ok(Fact {
        key: Some(row.key()),
        role,
        rank: row.int("rank"),
        title: row.text("title").map(str::to_string),
        body: body.to_string(),
        origin: row.text("origin").map(str::to_string),
    })
}
