use super::super::{BoundaryState, CheckoutState, Estate, IntegrationState, UpstreamState};
use super::{IssueWorktree, issue_stale};
use crate::{Error, PLUMB, Reference, Result, git};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueRelease {
    pub issue: super::super::super::Coordinate,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finish {
    pub issue: super::super::super::Coordinate,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueRetirement {
    pub issue: super::super::super::Coordinate,
    pub artifacts: Vec<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn finish(&self, request: &Finish) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let member = self.issue_member(&request.issue).await?;
        let path = self.issue_path(&anchor)?;
        let source = self.issue_source(&member)?;
        if !git::at(&path).clean()? {
            return Err(Error::typed(
                "concord.member.dirty",
                "Member has dirty or untracked files",
            ));
        }
        let head = git::at(&path).head()?;
        if head != member.base
            || git::at(&path).tree(&head)? != git::at(&path).tree(&member.base)?
        {
            return Err(Error::typed(
                "concord.member.changed",
                "Member has committed changes and must use delivery or guarded retirement",
            ));
        }
        if member.proof.is_some()
            || !self
                .issue_references(member.key, anchor.key)
                .await?
                .is_empty()
        {
            return Err(Error::typed(
                "concord.member.delivery_started",
                "Member has delivery state and cannot complete as unchanged research",
            ));
        }
        let claims = self
            .core
            .live("IssueClaim")
            .await
            .map_err(super::super::fault)?;
        let revision = anchor.revision + 1;
        let next = revision.to_string();
        git::at(&source).remove(&path)?;
        self.core
            .batch(async |tx| {
                for row in claims
                    .iter()
                    .filter(|row| row.int("member") == Some(member.key))
                {
                    tx.end("IssueClaim", row.key()).await?;
                }
                tx.end("IssueMember", member.key).await?;
                tx.set("Anchor", anchor.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(|error| {
                Error::typed(
                    "concord.member.disagreement",
                    format!("{error}; worktree is removed but estate still declares it"),
                )
            })?;
        prune(&path);
        Ok(revision)
    }

    pub async fn release_issue(&self, request: &IssueRelease) -> Result<i64> {
        self.finish_issue_member(&request.issue, request.revision, None)
            .await
    }

    pub async fn retire_issue(&self, request: &IssueRetirement) -> Result<i64> {
        self.finish_issue_member(&request.issue, request.revision, Some(&request.artifacts))
            .await
    }

    async fn finish_issue_member(
        &self,
        issue: &super::super::super::Coordinate,
        revision: i64,
        artifacts: Option<&[String]>,
    ) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(issue).await?;
        issue_stale(anchor.revision, revision)?;
        let member = self.issue_member(issue).await?;
        let path = self.issue_path(&anchor)?;
        let source = self.issue_source(&member)?;
        current(&member, &git::at(&path).head()?)?;
        if !git::at(&path).clean()? {
            return Err(Error::typed(
                "concord.member.dirty",
                "Member has dirty or untracked files",
            ));
        }
        if artifacts.is_none() && !git::at(&path).landed(&source)? {
            return Err(Error::typed(
                "concord.member.unlanded",
                "Member HEAD is neither reachable nor tree-equivalent to source HEAD",
            ));
        }
        if let Some(patterns) = artifacts {
            let held = self.issue_artifacts(issue).await?;
            if !held.iter().any(|artifact| {
                patterns
                    .iter()
                    .any(|pattern| pattern == "*" || pattern == &artifact.name)
            }) {
                return Err(Error::typed(
                    "concord.artifact.absent",
                    "no Artifact matches the given patterns",
                ));
            }
        }
        let claims = self
            .core
            .live("IssueClaim")
            .await
            .map_err(super::super::fault)?;
        let changes = self
            .core
            .live("IssueChange")
            .await
            .map_err(super::super::fault)?;
        let proof = member.proof.as_ref().expect("current checked proof");
        let revision = anchor.revision + 1;
        let next = revision.to_string();
        git::at(&source).remove(&path)?;
        let changed = self
            .core
            .batch(async |tx| {
                tx.end("IssueBoundary", proof.key).await?;
                for row in claims
                    .iter()
                    .filter(|row| row.int("member") == Some(member.key))
                {
                    tx.end("IssueClaim", row.key()).await?;
                }
                for row in changes
                    .iter()
                    .filter(|row| row.int("member") == Some(member.key))
                {
                    tx.end("IssueChange", row.key()).await?;
                }
                tx.end("IssueMember", member.key).await?;
                tx.set("Anchor", anchor.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await;
        if let Err(error) = changed {
            return Err(Error::typed(
                "concord.member.disagreement",
                format!("{error}; worktree is removed but estate still declares it"),
            ));
        }
        prune(&path);
        Ok(revision)
    }
}

fn prune(path: &std::path::Path) {
    if let Some(root) = path.parent()
        && root
            .read_dir()
            .ok()
            .and_then(|mut entries| entries.next())
            .is_none()
    {
        let _ = std::fs::remove_dir(root);
    }
}

fn current(member: &IssueWorktree, head: &str) -> Result<()> {
    let proof = member.proof.as_ref().ok_or_else(|| {
        Error::typed(
            "concord.boundary.absent",
            "Member has no current Boundary proof",
        )
    })?;
    let claim = crate::claim::digest(&member.claims);
    if !proof.current(head, &claim) {
        return Err(Error::typed(
            "concord.boundary.stale",
            "Member Boundary proof is stale",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IssueMemberStatus {
    pub member: IssueWorktree,
    pub references: Vec<Reference>,
    pub worktree: CheckoutState,
    pub integration_checkout: CheckoutState,
    pub boundary: BoundaryState,
    pub integration: IntegrationState,
    pub local_upstream: Option<UpstreamState>,
    pub local_tracking_refs: Vec<String>,
}

impl Estate {
    pub async fn issue_member_status(
        &self,
        issue: &super::super::super::Coordinate,
    ) -> Result<IssueMemberStatus> {
        let anchor = self.issue(issue).await?;
        let member = self.issue_member(issue).await?;
        let references = self.issue_references(member.key, anchor.key).await?;
        let path = self.issue_path(&anchor)?;
        let source = self.issue_source(&member)?;
        let worktree = super::super::status::checkout(&path)?;
        let integration_checkout = super::super::status::checkout(&source)?;
        let boundary = boundary(&member, &worktree.head);
        let integration =
            super::super::status::integration(&path, &source, &worktree, &integration_checkout)?;
        let local_upstream = super::super::status::upstream(&path, &member.branch)?;
        let local_tracking_refs = git::at(&path).tracking(&worktree.head)?;
        Ok(IssueMemberStatus {
            member,
            references,
            worktree,
            integration_checkout,
            boundary,
            integration,
            local_upstream,
            local_tracking_refs,
        })
    }
}

fn boundary(member: &IssueWorktree, head: &str) -> BoundaryState {
    let Some(proof) = &member.proof else {
        return BoundaryState::Absent;
    };
    let digest = crate::claim::digest(&member.claims);
    if [
        proof.schema == plumb::boundary::SCHEMA,
        proof.plumb == PLUMB,
        proof.head == head,
        proof.claim == digest,
    ]
    .into_iter()
    .all(|current| current)
    {
        BoundaryState::Current
    } else {
        BoundaryState::Stale
    }
}
