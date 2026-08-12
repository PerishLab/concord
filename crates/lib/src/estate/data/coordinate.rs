use super::super::task::reserved;
use super::super::{Estate, Life, World, fault};
use crate::git;
use crate::model::component;
use crate::path::at;
use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rename {
    pub task: String,
    pub name: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rehome {
    pub task: String,
    pub domain: String,
    pub revision: i64,
}

struct Move {
    from: PathBuf,
    to: PathBuf,
    repairs: Vec<(PathBuf, String)>,
}

impl Estate {
    pub async fn rename(&self, rename: &Rename) -> Result<super::super::Node> {
        component("task name", &rename.name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&rename.task)?;
        active(task.life, &rename.task)?;
        stale(task.revision, rename.revision)?;
        let domain = world.domain(&task.domain).expect("Task Domain is loaded");
        available(self, domain, &task.domain, &rename.name).await?;
        let from = self
            .space
            .join(&task.domain)
            .join(".tasks")
            .join(&task.name);
        let to = self
            .space
            .join(&task.domain)
            .join(".tasks")
            .join(&rename.name);
        let moved = self.moving(task, from, to).await?;
        moved.apply()?;
        let revision = task.revision + 1;
        let domain = domain.to_string();
        let next = revision.to_string();
        let changed = self
            .core
            .batch(async |tx| {
                tx.put(
                    "Reservation",
                    &[("name", rename.name.as_str()), ("domain", domain.as_str())],
                )
                .await?;
                tx.set(
                    "Task",
                    task.key,
                    &[("name", rename.name.as_str()), ("revision", next.as_str())],
                )
                .await?;
                Ok(())
            })
            .await;
        if let Err(error) = changed {
            return moved.rollback(fault(error));
        }
        let mut task = task.clone();
        task.name = rename.name.clone();
        task.revision = revision;
        Ok(task)
    }

    pub async fn rehome(&self, rehome: &Rehome) -> Result<super::super::Node> {
        component("domain name", &rehome.domain)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&rehome.task)?;
        active(task.life, &rehome.task)?;
        stale(task.revision, rehome.revision)?;
        if task.domain == rehome.domain {
            return Err(Error::typed(
                "concord.task.domain",
                "Task already belongs to the target Domain",
            ));
        }
        let domain = world.domain(&rehome.domain).ok_or_else(|| {
            Error::typed(
                "concord.domain.absent",
                format!("unknown managed domain {}", rehome.domain),
            )
        })?;
        available(self, domain, &rehome.domain, &task.name).await?;
        let from = self
            .space
            .join(&task.domain)
            .join(".tasks")
            .join(&task.name);
        let to = self
            .space
            .join(&rehome.domain)
            .join(".tasks")
            .join(&task.name);
        let members = self
            .worktrees()
            .await?
            .into_iter()
            .filter(|member| member.task == rehome.task)
            .map(|member| (member.key, self.source(&member)))
            .collect::<Vec<_>>();
        let moved = self.moving(task, from, to).await?;
        moved.apply()?;
        let revision = task.revision + 1;
        let root = domain.to_string();
        let next = revision.to_string();
        let changed = self
            .core
            .batch(async |tx| {
                tx.put(
                    "Reservation",
                    &[("name", task.name.as_str()), ("domain", root.as_str())],
                )
                .await?;
                tx.set(
                    "Task",
                    task.key,
                    &[("domain", root.as_str()), ("revision", next.as_str())],
                )
                .await?;
                for (key, source) in &members {
                    let source = source
                        .as_ref()
                        .map_err(|error| keel::adapt::Error::Adapt(error.to_string()))?;
                    tx.set(
                        "Member",
                        *key,
                        &[("source", source.to_string_lossy().as_ref())],
                    )
                    .await?;
                }
                Ok(())
            })
            .await;
        if let Err(error) = changed {
            return moved.rollback(fault(error));
        }
        let mut task = task.clone();
        task.domain = rehome.domain.clone();
        task.revision = revision;
        Ok(task)
    }

    async fn moving(&self, task: &super::super::Node, from: PathBuf, to: PathBuf) -> Result<Move> {
        if to.exists() || std::fs::symlink_metadata(&to).is_ok() {
            return Err(Error::typed(
                "concord.task.territory",
                format!("target Task seat is occupied: {}", to.display()),
            ));
        }
        let repairs = self
            .worktrees()
            .await?
            .into_iter()
            .filter(|member| member.task == task.identity())
            .map(|member| Ok((self.source(&member)?, member.name)))
            .collect::<Result<Vec<_>>>()?;
        Ok(Move { from, to, repairs })
    }
}

impl Move {
    fn apply(&self) -> Result<()> {
        if !self.from.exists() {
            return Ok(());
        }
        let parent = self
            .to
            .parent()
            .ok_or_else(|| Error::new("target Task seat has no parent"))?;
        at(parent).directory()?;
        std::fs::rename(&self.from, &self.to)?;
        if let Err(error) = self.repair(&self.to) {
            let _ = std::fs::rename(&self.to, &self.from);
            let _ = self.repair(&self.from);
            return Err(error);
        }
        Ok(())
    }

    fn rollback<T>(&self, error: Error) -> Result<T> {
        if self.to.exists() {
            std::fs::rename(&self.to, &self.from).map_err(|rollback| {
                Error::typed(
                    "concord.task.disagreement",
                    format!("{error}; Task seat rollback failed: {rollback}"),
                )
            })?;
            self.repair(&self.from).map_err(|rollback| {
                Error::typed(
                    "concord.task.disagreement",
                    format!("{error}; worktree repair rollback failed: {rollback}"),
                )
            })?;
        }
        Err(error)
    }

    fn repair(&self, root: &Path) -> Result<()> {
        for (source, name) in &self.repairs {
            git::at(source).repair(&root.join(name))?;
        }
        Ok(())
    }
}

fn active(life: Life, task: &str) -> Result<()> {
    if life == Life::Active {
        return Ok(());
    }
    Err(Error::typed(
        "concord.task.retired",
        format!("retired Task is immutable: {task}"),
    ))
}

fn stale(found: i64, expected: i64) -> Result<()> {
    if found == expected {
        return Ok(());
    }
    Err(Error::typed(
        "concord.task.stale",
        format!("Task revision changed: expected {expected}, found {found}"),
    ))
}

async fn available(estate: &Estate, key: i64, domain: &str, name: &str) -> Result<()> {
    if reserved(estate, key, name).await? {
        return Err(Error::typed(
            "concord.task.reserved",
            format!("Task name is already reserved: {domain}/{name}"),
        ));
    }
    Ok(())
}
