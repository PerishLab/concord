mod artifact;
pub mod authority;
mod claim;
pub mod delivery;
pub mod landing;
mod lifecycle;
mod proof;
mod reference;
mod start;

use super::super::{Anchor, Coordinate, Estate, fault};
use crate::{Error, Result, component};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub use artifact::{IssueArtifact, IssueImport, IssueRemoval};
pub use claim::{IssueClaiming, IssueMemberChange, IssueNarrowing};
pub use lifecycle::{Finish, IssueMemberStatus, IssueRelease, IssueRetirement};
pub use proof::IssueProving;
pub use reference::{IssueDeclaration, IssueWithdrawal};
pub use start::Start;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IssueWorktree {
    pub key: i64,
    pub issue: Coordinate,
    pub node: String,
    pub integration: super::Integration,
    pub branch: String,
    pub base: String,
    pub claims: Vec<String>,
    pub proof: Option<super::Proof>,
}

impl Estate {
    pub async fn issue_worktrees(&self) -> Result<Vec<IssueWorktree>> {
        let anchors = self.core.live("Anchor").await.map_err(fault)?;
        let integrations = self.core.live("Integration").await.map_err(fault)?;
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
            let integration = row
                .int("integration")
                .and_then(|key| integrations.iter().find(|held| held.key() == key))
                .map(super::integration::decode)
                .transpose()?
                .ok_or_else(|| malformed(row.key(), "integration"))?;
            let branch = text(&row, "branch")?;
            let base = text(&row, "base")?;
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
                integration,
                branch,
                base,
                claims: held,
                proof: issue_proof(&proofs, row.key(), anchor.key)?,
            });
        }
        out.sort_by_key(|member| member.node.clone());
        Ok(out)
    }

    pub async fn issue_member(&self, issue: &Coordinate) -> Result<IssueWorktree> {
        let anchor = self.issue(issue).await?;
        self.issue_worktrees()
            .await?
            .into_iter()
            .find(|member| member.node == anchor.node)
            .ok_or_else(|| {
                Error::typed(
                    "concord.member.absent",
                    format!("member not found: {}", issue.identity()),
                )
            })
    }

    pub async fn issue_source_path(&self, issue: &Coordinate) -> Result<PathBuf> {
        let member = self.issue_member(issue).await?;
        self.issue_source(&member)
    }

    pub(in crate::estate) fn issue_path(&self, anchor: &Anchor) -> Result<PathBuf> {
        component("issue node", &anchor.node)?;
        Ok(self
            .space
            .join(".issues")
            .join(&anchor.node)
            .join("worktree"))
    }

    pub(in crate::estate) fn issue_source(&self, member: &IssueWorktree) -> Result<PathBuf> {
        Ok(PathBuf::from(&member.integration.path))
    }
}

pub(super) fn branch(node: &str) -> String {
    let digest = Sha256::digest(node.as_bytes());
    format!("concord/issue-{digest:x}")
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
