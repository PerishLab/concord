use super::super::{ClaimOverlap, Estate};
use super::{IssueWorktree, issue_stale};
use crate::{Error, Result, git};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueClaiming {
    pub issue: super::super::super::Coordinate,
    pub member: String,
    pub claims: Vec<String>,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueNarrowing {
    pub issue: super::super::super::Coordinate,
    pub member: String,
    pub claims: Vec<String>,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IssueMemberChange {
    pub member: IssueWorktree,
    pub observations: Vec<ClaimOverlap>,
}

struct Change<'a> {
    issue: &'a super::super::super::Coordinate,
    member: &'a str,
    claims: &'a [String],
    revision: i64,
    narrow: bool,
}

impl Estate {
    pub fn claim_intersections(
        &self,
        left: &IssueWorktree,
        right: &IssueWorktree,
    ) -> Result<Vec<String>> {
        if git::at(&self.issue_source(left)?).identity()?
            != git::at(&self.issue_source(right)?).identity()?
        {
            return Ok(Vec::new());
        }
        Ok(crate::claim::intersections(&left.claims, &right.claims))
    }

    pub async fn claim_issue(&self, request: &IssueClaiming) -> Result<IssueMemberChange> {
        self.change_issue_claim(Change {
            issue: &request.issue,
            member: &request.member,
            claims: &request.claims,
            revision: request.revision,
            narrow: false,
        })
        .await
    }

    pub async fn narrow_issue(&self, request: &IssueNarrowing) -> Result<IssueMemberChange> {
        self.change_issue_claim(Change {
            issue: &request.issue,
            member: &request.member,
            claims: &request.claims,
            revision: request.revision,
            narrow: true,
        })
        .await
    }

    async fn change_issue_claim(&self, change: Change<'_>) -> Result<IssueMemberChange> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(change.issue).await?;
        issue_stale(anchor.revision, change.revision)?;
        let member = self.issue_member(change.issue, change.member).await?;
        let claims = if change.narrow {
            crate::claim::normalize(change.claims)?
        } else {
            let mut union = member.claims.clone();
            union.extend(change.claims.iter().cloned());
            crate::claim::normalize(&union)?
        };
        if claims == member.claims {
            return Err(Error::typed(
                "concord.claim.unchanged",
                "claim change does not change the normalized claim",
            ));
        }
        let source = self.issue_source(&member)?;
        let observations = self
            .issue_overlaps(Some(member.key), &source, &claims)
            .await?;
        if change.narrow {
            let path = self.issue_path(&anchor, &member.name)?;
            if !git::at(&source).clean()? {
                return Err(Error::typed(
                    "concord.member.source",
                    "integration checkout is not clean",
                ));
            }
            let head = git::at(&path).head()?;
            let origin = git::at(&source).head()?;
            let base = git::at(&path).merge(&head, &origin)?;
            let report = plumb::boundary::check(plumb::boundary::Request {
                root: &path,
                base: &base,
                head: &head,
                write: &claims,
            })
            .map_err(|error| Error::typed("concord.boundary.refused", error.to_string()))?;
            if !report.ok {
                return Err(Error::typed(
                    "concord.boundary.outside",
                    format!("changes outside claim: {}", report.outside.join(", ")),
                ));
            }
        }
        let rows = self
            .core
            .live("IssueClaim")
            .await
            .map_err(super::super::fault)?;
        let boundary = self
            .core
            .live("IssueBoundary")
            .await
            .map_err(super::super::fault)?
            .into_iter()
            .find(|row| row.int("member") == Some(member.key));
        let next = (anchor.revision + 1).to_string();
        let parent = member.key.to_string();
        let root = anchor.key.to_string();
        self.core
            .batch(async |tx| {
                for row in rows
                    .iter()
                    .filter(|row| row.int("member") == Some(member.key))
                {
                    tx.end("IssueClaim", row.key()).await?;
                }
                if let Some(boundary) = &boundary {
                    tx.end("IssueBoundary", boundary.key()).await?;
                }
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
                Ok(())
            })
            .await
            .map_err(super::super::fault)?;
        Ok(IssueMemberChange {
            member: self.issue_member(change.issue, change.member).await?,
            observations,
        })
    }

    pub(super) async fn issue_overlaps(
        &self,
        owner: Option<i64>,
        source: &Path,
        claims: &[String],
    ) -> Result<Vec<ClaimOverlap>> {
        let identity = git::at(source).identity()?;
        let mut observations = Vec::new();
        for member in self.issue_worktrees().await? {
            if owner == Some(member.key) {
                continue;
            }
            let held = self.issue_source(&member)?;
            if git::at(&held).identity()? == identity {
                let paths = crate::claim::intersections(claims, &member.claims);
                if !paths.is_empty() {
                    observations.push(ClaimOverlap {
                        code: "claim.overlap".to_string(),
                        peer: format!("{}/{}", member.issue.identity(), member.name),
                        paths,
                    });
                }
            }
        }
        observations.sort_by_key(|observation| observation.peer.clone());
        Ok(observations)
    }
}
