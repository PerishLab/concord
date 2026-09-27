mod artifact;
mod claim;
pub mod landing;
mod proof;
mod reference;
mod release;
mod status;

use super::super::{Anchor, Coordinate, Estate, fault};
use crate::path::at;
use crate::{Error, Result, component, git};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub use artifact::{IssueArtifact, IssueImport, IssueRemoval};
pub use claim::{IssueClaiming, IssueMemberChange, IssueNarrowing};
pub use proof::IssueProving;
pub use reference::{IssueDeclaration, IssueWithdrawal};
pub use release::{IssueRelease, IssueRetirement};
pub use status::IssueMemberStatus;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueAttach {
    pub issue: Coordinate,
    pub name: String,
    pub source: PathBuf,
    pub branch: Option<String>,
    pub claims: Vec<String>,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IssueWorktree {
    pub key: i64,
    pub issue: Coordinate,
    pub node: String,
    pub name: String,
    pub source: String,
    pub branch: String,
    pub claims: Vec<String>,
    pub proof: Option<super::Proof>,
}

impl Estate {
    pub async fn issue_worktrees(&self) -> Result<Vec<IssueWorktree>> {
        self.executable()?;
        let anchors = self.core.live("Anchor").await.map_err(fault)?;
        let claims = self.core.live("IssueClaim").await.map_err(fault)?;
        let proofs = self.core.live("IssueBoundary").await.map_err(fault)?;
        let mut out = Vec::new();
        for row in self.core.live("IssueMember").await.map_err(fault)? {
            let anchor = row
                .int("anchor")
                .and_then(|key| anchors.iter().find(|anchor| anchor.key() == key))
                .map(super::super::forge::decode_anchor)
                .transpose()?
                .ok_or_else(|| malformed(row.key(), "anchor"))?;
            let name = text(&row, "name")?;
            let source = text(&row, "source")?;
            let branch = text(&row, "branch")?;
            let mut held = claims
                .iter()
                .filter(|claim| claim.int("member") == Some(row.key()))
                .map(|claim| {
                    if claim.int("anchor") != Some(anchor.key) {
                        return Err(malformed(claim.key(), "anchor"));
                    }
                    text(claim, "path")
                })
                .collect::<Result<Vec<_>>>()?;
            held.sort();
            out.push(IssueWorktree {
                key: row.key(),
                issue: anchor.coordinate,
                node: anchor.node,
                name,
                source,
                branch,
                claims: held,
                proof: issue_proof(&proofs, row.key(), anchor.key)?,
            });
        }
        out.sort_by_key(|member| (member.node.clone(), member.name.clone()));
        Ok(out)
    }

    pub async fn attach_issue(&self, request: &IssueAttach) -> Result<IssueMemberChange> {
        self.executable()?;
        component("member name", &request.name)?;
        let source = request.source.canonicalize().map_err(|error| {
            Error::typed(
                "concord.member.source",
                format!(
                    "cannot resolve source {}: {error}",
                    request.source.display()
                ),
            )
        })?;
        if !git::at(&source).clean()? {
            return Err(Error::typed(
                "concord.member.source",
                "integration checkout is not clean",
            ));
        }
        let claims = crate::claim::normalize(&request.claims)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let branch = request
            .branch
            .clone()
            .unwrap_or_else(|| format!("issue-{}-{}", anchor.key, request.name));
        if git::at(&source).exists(&branch)? {
            return Err(Error::typed(
                "concord.member.branch",
                format!("target branch already exists: {branch}"),
            ));
        }
        if self
            .issue_worktrees()
            .await?
            .iter()
            .any(|member| member.node == anchor.node && member.name == request.name)
        {
            return Err(Error::typed(
                "concord.member.reserved",
                format!(
                    "member already exists: {}/{}",
                    request.issue.identity(),
                    request.name
                ),
            ));
        }
        let observations = self.issue_overlaps(None, &source, &claims).await?;
        let path = self.issue_path(&anchor, &request.name)?;
        if path.exists() {
            return Err(Error::typed(
                "concord.member.territory",
                format!("member path already exists: {}", path.display()),
            ));
        }
        let root = path
            .parent()
            .ok_or_else(|| Error::new("member path has no Issue parent"))?;
        let fresh = !root.exists();
        at(root).directory()?;
        if let Err(error) = git::at(&source).add(&path, &branch, false) {
            if fresh {
                let _ = std::fs::remove_dir(root);
            }
            return Err(error);
        }
        let stored = source.display().to_string();
        let revision = anchor.revision + 1;
        let next = revision.to_string();
        let root = anchor.key.to_string();
        let made = self
            .core
            .batch(async |tx| {
                let member = tx
                    .put(
                        "IssueMember",
                        &[
                            ("name", request.name.as_str()),
                            ("source", stored.as_str()),
                            ("branch", branch.as_str()),
                            ("anchor", root.as_str()),
                        ],
                    )
                    .await?;
                let parent = member.to_string();
                for claim in &claims {
                    tx.put(
                        "IssueClaim",
                        &[
                            ("path", claim.as_str()),
                            ("member", parent.as_str()),
                            ("anchor", root.as_str()),
                        ],
                    )
                    .await?;
                }
                tx.set("Anchor", anchor.key, &[("revision", next.as_str())])
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
                    format!("{error}; worktree rollback failed: {rollback}"),
                )),
            };
        }
        Ok(IssueMemberChange {
            member: self.issue_member(&request.issue, &request.name).await?,
            observations,
        })
    }

    pub async fn issue_member(&self, issue: &Coordinate, name: &str) -> Result<IssueWorktree> {
        let anchor = self.issue(issue).await?;
        self.issue_worktrees()
            .await?
            .into_iter()
            .find(|member| member.node == anchor.node && member.name == name)
            .ok_or_else(|| {
                Error::typed(
                    "concord.member.absent",
                    format!("member not found: {}/{}", issue.identity(), name),
                )
            })
    }

    pub async fn issue_source_path(&self, issue: &Coordinate, name: &str) -> Result<PathBuf> {
        let member = self.issue_member(issue, name).await?;
        self.issue_source(&member)
    }

    pub(in crate::estate) fn issue_path(&self, anchor: &Anchor, name: &str) -> Result<PathBuf> {
        component("issue node", &anchor.node)?;
        Ok(self
            .space
            .join(".issues")
            .join(&anchor.node)
            .join("members")
            .join(name))
    }

    pub(in crate::estate) fn issue_source(&self, member: &IssueWorktree) -> Result<PathBuf> {
        crate::path::expand(
            &member.source,
            &self.space.join(".issues").join(&member.node),
        )
    }

    pub(super) fn executable(&self) -> Result<()> {
        if self.execution {
            return Ok(());
        }
        Err(Error::typed(
            "concord.issue.execution_migration_required",
            "this estate has no Issue execution resources; follow the explicit estate transition",
        ))
    }
}

fn issue_proof(rows: &[keel::Row], member: i64, anchor: i64) -> Result<Option<super::Proof>> {
    let Some(row) = rows.iter().find(|row| row.int("member") == Some(member)) else {
        return Ok(None);
    };
    if row.int("anchor") != Some(anchor) {
        return Err(malformed(row.key(), "anchor"));
    }
    Ok(Some(super::Proof {
        key: row.key(),
        schema: text(row, "schema")?,
        plumb: text(row, "plumb")?,
        base: text(row, "base")?,
        head: text(row, "head")?,
        claim: text(row, "claim")?,
    }))
}

pub(super) fn issue_stale(found: i64, expected: i64) -> Result<()> {
    if found == expected {
        return Ok(());
    }
    Err(Error::typed(
        "concord.issue.stale",
        format!("Issue execution revision changed: expected {expected}, found {found}"),
    ))
}

pub(super) fn text(row: &keel::Row, field: &str) -> Result<String> {
    row.text(field)
        .map(str::to_string)
        .ok_or_else(|| malformed(row.key(), field))
}

pub(super) fn malformed(key: i64, field: &str) -> Error {
    Error::typed(
        "concord.member.row",
        format!("Issue execution Resource {key} has malformed field {field}"),
    )
}

pub(super) fn direct(path: &Path) -> bool {
    path.is_dir()
        && std::fs::symlink_metadata(path)
            .map(|metadata| !metadata.file_type().is_symlink())
            .unwrap_or(false)
}
