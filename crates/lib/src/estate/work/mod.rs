mod artifact;
mod claim;
mod copy;
mod narrow;
mod proof;
mod release;
mod status;
use super::{Estate, Life, World, fault};
use crate::path::at;
use crate::{Error, Result, component, git};
pub use artifact::{Artifact, Import, Removal, Survey};
pub use claim::Claiming;
pub use narrow::Narrowing;
pub use proof::Proving;
pub use release::{Release, Retirement};
use serde::Serialize;
pub use status::{BoundaryState, CheckoutState, IntegrationState, MemberStatus, UpstreamState};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attach {
    pub task: String,
    pub name: String,
    pub source: PathBuf,
    pub branch: Option<String>,
    pub claims: Vec<String>,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Proof {
    pub key: i64,
    pub schema: String,
    pub plumb: String,
    pub base: String,
    pub head: String,
    pub claim: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Worktree {
    pub key: i64,
    pub task: String,
    pub name: String,
    pub source: String,
    pub branch: String,
    pub claims: Vec<String>,
    pub proof: Option<Proof>,
}

impl Estate {
    pub async fn worktrees(&self) -> Result<Vec<Worktree>> {
        let world = World::load(self).await?;
        let claims = self.core.live("Claim").await.map_err(fault)?;
        let proofs = self.core.live("Boundary").await.map_err(fault)?;
        let mut out = Vec::new();
        for row in self.core.live("Member").await.map_err(fault)? {
            let task = row.int("task").and_then(|key| {
                world
                    .nodes
                    .iter()
                    .find(|node| node.key == key)
                    .map(|node| node.identity())
            });
            let task = task.ok_or_else(|| malformed(row.key(), "task"))?;
            let name = row
                .text("name")
                .ok_or_else(|| malformed(row.key(), "name"))?;
            let source = row
                .text("source")
                .ok_or_else(|| malformed(row.key(), "source"))?;
            let fallback = task.split_once('/').map(|pair| pair.1).unwrap_or("");
            let branch = row.text("branch").unwrap_or(fallback).to_string();
            let mut held = claims
                .iter()
                .filter(|claim| claim.int("member") == Some(row.key()))
                .filter_map(|claim| claim.text("path").map(str::to_string))
                .collect::<Vec<_>>();
            held.sort();
            out.push(Worktree {
                key: row.key(),
                task,
                name: name.to_string(),
                source: source.to_string(),
                branch,
                claims: held,
                proof: proof(&proofs, row.key())?,
            });
        }
        out.sort_by_key(|member| (member.task.clone(), member.name.clone()));
        Ok(out)
    }

    pub async fn attach(&self, attach: &Attach) -> Result<Worktree> {
        component("member name", &attach.name)?;
        let source = attach.source.canonicalize().map_err(|error| {
            Error::typed(
                "concord.member.source",
                format!("cannot resolve source {}: {error}", attach.source.display()),
            )
        })?;
        if !git::at(&source).clean()? {
            return Err(Error::typed(
                "concord.member.source",
                "integration checkout is not clean",
            ));
        }
        let claims = crate::claim::normalize(&attach.claims)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&attach.task)?;
        active(task.life, &attach.task)?;
        stale(task.revision, attach.revision)?;
        let identity = task.identity();
        let branch = attach.branch.as_deref().unwrap_or(&task.name);
        if git::at(&source).exists(branch)? {
            return Err(Error::typed(
                "concord.member.branch",
                format!("target branch already exists: {branch}"),
            ));
        }
        self.available(None, &source, &claims).await?;
        if self
            .worktrees()
            .await?
            .iter()
            .any(|member| member.task == attach.task && member.name == attach.name)
        {
            return Err(Error::typed(
                "concord.member.reserved",
                format!("member already exists: {}/{}", attach.task, attach.name),
            ));
        }
        let path = self.path(task.domain.as_str(), task.name.as_str(), &attach.name);
        if path.exists() {
            return Err(Error::typed(
                "concord.member.territory",
                format!("member path already exists: {}", path.display()),
            ));
        }
        let root = path
            .parent()
            .ok_or_else(|| Error::new("member path has no Task parent"))?;
        let fresh = !root.exists();
        at(&self.space.join(&task.domain).join(".tasks")).directory()?;
        at(root).directory()?;
        if let Err(error) = git::at(&source).add(&path, branch, false) {
            if fresh {
                let _ = std::fs::remove_dir(root);
            }
            return Err(error);
        }
        let stored = source.display().to_string();
        let revision = task.revision + 1;
        let next = revision.to_string();
        let root = task.key.to_string();
        let made = self
            .core
            .batch(async |tx| {
                let member = tx
                    .put(
                        "Member",
                        &[
                            ("name", attach.name.as_str()),
                            ("source", stored.as_str()),
                            ("branch", branch),
                            ("task", root.as_str()),
                        ],
                    )
                    .await?;
                let parent = member.to_string();
                for claim in &claims {
                    tx.put(
                        "Claim",
                        &[("path", claim.as_str()), ("member", parent.as_str())],
                    )
                    .await?;
                }
                tx.set("Task", task.key, &[("revision", next.as_str())])
                    .await?;
                Ok(member)
            })
            .await;
        if let Err(error) = made {
            let rollback = git::at(&source).remove(&path);
            if fresh {
                let _ = std::fs::remove_dir(root);
            }
            return match rollback {
                Ok(()) => Err(fault(error)),
                Err(rollback) => Err(Error::typed(
                    "concord.member.disagreement",
                    format!("{}; worktree rollback failed: {rollback}", error),
                )),
            };
        }
        self.member(&identity, &attach.name).await
    }

    pub(super) async fn member(&self, task: &str, name: &str) -> Result<Worktree> {
        self.worktrees()
            .await?
            .into_iter()
            .find(|member| member.task == task && member.name == name)
            .ok_or_else(|| {
                Error::typed(
                    "concord.member.absent",
                    format!("member not found: {task}/{name}"),
                )
            })
    }

    pub(super) fn path(&self, domain: &str, task: &str, name: &str) -> PathBuf {
        self.space.join(domain).join(".tasks").join(task).join(name)
    }

    pub(super) fn source(&self, member: &Worktree) -> Result<PathBuf> {
        let domain = member
            .task
            .split_once('/')
            .map(|pair| pair.0)
            .ok_or_else(|| {
                Error::typed(
                    "concord.member.task",
                    format!("Member has malformed Task identity: {}", member.task),
                )
            })?;
        crate::path::expand(&member.source, &self.space.join(domain).join(".tasks"))
    }

    async fn available(&self, owner: Option<i64>, source: &Path, claims: &[String]) -> Result<()> {
        let identity = git::at(source).identity()?;
        for member in self.worktrees().await? {
            if owner == Some(member.key) {
                continue;
            }
            let held = self.source(&member)?;
            if git::at(&held).identity()? == identity
                && crate::claim::overlaps(claims, &member.claims)
            {
                return Err(Error::typed(
                    "concord.claim.overlap",
                    format!(
                        "write claim overlaps active member {}/{}",
                        member.task, member.name
                    ),
                ));
            }
        }
        Ok(())
    }
}

fn proof(rows: &[keel::Row], member: i64) -> Result<Option<Proof>> {
    let Some(row) = rows.iter().find(|row| row.int("member") == Some(member)) else {
        return Ok(None);
    };
    let text = |field| {
        row.text(field)
            .map(str::to_string)
            .ok_or_else(|| malformed(row.key(), field))
    };
    Ok(Some(Proof {
        key: row.key(),
        schema: text("schema")?,
        plumb: text("plumb")?,
        base: text("base")?,
        head: text("head")?,
        claim: text("claim")?,
    }))
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

fn malformed(key: i64, field: &str) -> Error {
    Error::typed(
        "concord.member.row",
        format!("Member Resource {key} has malformed field {field}"),
    )
}
