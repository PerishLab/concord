use super::super::overlap::{Subject, committed, same};
use super::super::{Committed, Estate, Evidence, Overlap};
use super::{Worktree, stale};
use crate::estate::Scope;
use crate::{Error, Result, git};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claiming {
    pub issue: super::super::super::Coordinate,
    pub claims: Vec<String>,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Narrowing {
    pub issue: super::super::super::Coordinate,
    pub claims: Vec<String>,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Change {
    pub member: Worktree,
    pub observations: Vec<Overlap>,
}

struct Edit<'a> {
    issue: &'a super::super::super::Coordinate,
    claims: &'a [String],
    revision: i64,
    narrow: bool,
}

impl Estate {
    pub fn intersections(&self, left: &Worktree, right: &Worktree) -> Result<Vec<String>> {
        if git::at(&self.source(left)?).identity()? != git::at(&self.source(right)?).identity()? {
            return Ok(Vec::new());
        }
        Ok(crate::claim::intersections(&left.claims, &right.claims))
    }

    pub async fn claim(&self, request: &Claiming) -> Result<Change> {
        self.update(Edit {
            issue: &request.issue,
            claims: &request.claims,
            revision: request.revision,
            narrow: false,
        })
        .await
    }

    pub async fn narrow(&self, request: &Narrowing) -> Result<Change> {
        self.update(Edit {
            issue: &request.issue,
            claims: &request.claims,
            revision: request.revision,
            narrow: true,
        })
        .await
    }

    async fn update(&self, change: Edit<'_>) -> Result<Change> {
        let _guard = self.guard()?;
        self.ensure(Scope::Issue(change.issue)).await?;
        let anchor = self.issue(change.issue).await?;
        stale(anchor.revision, change.revision)?;
        let member = self.member(change.issue).await?;
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
        let source = self.source(&member)?;
        let subject = Subject {
            key: Some(member.key),
            identity: member.issue.identity(),
            path: self.seat(&anchor)?,
            base: member.base.clone(),
            claims: claims.clone(),
            proof: member.proof.clone(),
        };
        let observations = self.overlaps(&subject, &source).await?;
        if change.narrow {
            let path = self.seat(&anchor)?;
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
        Ok(Change {
            member: self.member(change.issue).await?,
            observations,
        })
    }

    pub(super) async fn overlaps(&self, subject: &Subject, source: &Path) -> Result<Vec<Overlap>> {
        let mut observations = Vec::new();
        let mut identities = git::Identities::default();
        let common = git::at(source).identity()?;
        for member in self.worktrees().await? {
            if subject.key == Some(member.key)
                || std::path::Path::new(&member.integration.common) != common
            {
                continue;
            }
            let held = self.source(&member)?;
            if !same(&mut identities, source, &held)? {
                continue;
            }
            let paths = crate::claim::intersections(&subject.claims, &member.claims);
            if paths.is_empty() {
                continue;
            }
            let anchor = self.issue(&member.issue).await?;
            let peer = Subject {
                key: Some(member.key),
                identity: member.issue.identity(),
                path: self.seat(&anchor)?,
                base: member.base.clone(),
                claims: member.claims.clone(),
                proof: member.proof.clone(),
            };
            let committed =
                if git::at(&peer.path).branch().ok().as_deref() == Some(member.branch.as_str()) {
                    committed(subject, &peer)
                } else {
                    Committed::Unavailable {
                        member: peer.identity.clone(),
                        reason: Evidence::Unavailable,
                    }
                };
            observations.push(Overlap {
                code: "claim.overlap".to_string(),
                peer: peer.identity.clone(),
                paths,
                committed,
            });
        }
        observations.sort_by_key(|observation| observation.peer.clone());
        Ok(observations)
    }
}
