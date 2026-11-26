use super::super::{Boundary, Checkout, Estate, Relation, Upstream};
use crate::estate::Scope;
mod cleanup;
mod recovery;

use super::{Worktree, stale};
use crate::{Error, Reference, Result, git};
use cleanup::Cleanup;
pub use recovery::Recovery;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    pub issue: super::super::super::Coordinate,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finish {
    pub issue: super::super::super::Coordinate,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Retirement {
    pub issue: super::super::super::Coordinate,
    pub artifacts: Vec<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn finish(&self, request: &Finish) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure(Scope::Issue(&request.issue)).await?;
        let anchor = self.issue(&request.issue).await?;
        stale(anchor.revision, request.revision)?;
        let member = self.member(&request.issue).await?;
        let path = self.seat(&anchor)?;
        let source = self.source(&member)?;
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
        if member.proof.is_some() || !self.references(member.key, anchor.key).await?.is_empty() {
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
        let cleanup = Cleanup {
            source: &source,
            path: &path,
            branch: &member.branch,
            head: &head,
        };
        cleanup
            .complete(async {
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
                    .map_err(super::super::fault)
            })
            .await?;
        prune(&path);
        Ok(revision)
    }

    pub async fn release(&self, request: &Release) -> Result<i64> {
        self.settle(&request.issue, request.revision, None).await
    }

    pub async fn dismiss(&self, request: &Retirement) -> Result<i64> {
        self.settle(&request.issue, request.revision, Some(&request.artifacts))
            .await
    }

    async fn settle(
        &self,
        issue: &super::super::super::Coordinate,
        revision: i64,
        artifacts: Option<&[String]>,
    ) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure(Scope::Issue(issue)).await?;
        let anchor = self.issue(issue).await?;
        stale(anchor.revision, revision)?;
        let member = self.member(issue).await?;
        let path = self.seat(&anchor)?;
        let source = self.source(&member)?;
        let head = git::at(&path).head()?;
        current(&member, &head)?;
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
            let held = self.artifacts(issue).await?;
            if !held.iter().any(|artifact| {
                patterns
                    .iter()
                    .any(|pattern| pattern == "*" || pattern == &artifact.name)
            }) {
                return Err(Error::typed(
                    "concord.artifact.absent",
                    "no Artifact matches the given patterns; preserve the unlanded work with concord artifact import, then select matching Issue-held Artifact names with concord member retire --artifacts",
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
        let cleanup = Cleanup {
            source: &source,
            path: &path,
            branch: &member.branch,
            head: &head,
        };
        cleanup
            .settle(async {
                self.core
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
                    .await
                    .map_err(super::super::fault)
            })
            .await?;
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

fn current(member: &Worktree, head: &str) -> Result<()> {
    let proof = member.proof.as_ref().ok_or_else(|| {
        Error::typed(
            "concord.boundary.absent",
            "Member has no current Boundary proof",
        )
    })?;
    let claim = crate::claim::digest(&member.claims);
    if let Some(reason) = proof.stale(head, &claim) {
        return Err(Error::typed("concord.boundary.stale", reason));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Status {
    pub member: Worktree,
    pub references: Vec<Reference>,
    pub worktree: Checkout,
    #[serde(rename = "integration_checkout")]
    pub checkout: Checkout,
    pub boundary: Boundary,
    pub integration: Relation,
    #[serde(rename = "local_upstream")]
    pub upstream: Option<Upstream>,
    #[serde(rename = "local_tracking_refs")]
    pub tracking: Vec<String>,
}

impl Estate {
    pub async fn status(&self, issue: &super::super::super::Coordinate) -> Result<Status> {
        let anchor = self.issue(issue).await?;
        let member = self.member(issue).await?;
        let references = self.references(member.key, anchor.key).await?;
        let path = self.seat(&anchor)?;
        let source = self.source(&member)?;
        let worktree = super::super::status::checkout(&path)?;
        let checkout = super::super::status::checkout(&source)?;
        let boundary = boundary(&member, &worktree.head);
        let integration = super::super::status::integration(&path, &source, &worktree, &checkout)?;
        let upstream = super::super::status::upstream(&path, &member.branch)?;
        let tracking = git::at(&path).tracking(&worktree.head)?;
        Ok(Status {
            member,
            references,
            worktree,
            checkout,
            boundary,
            integration,
            upstream,
            tracking,
        })
    }
}

fn boundary(member: &Worktree, head: &str) -> Boundary {
    let Some(proof) = &member.proof else {
        return Boundary::Absent;
    };
    if proof.current(head, &crate::claim::digest(&member.claims)) {
        Boundary::Current
    } else {
        Boundary::Stale
    }
}
