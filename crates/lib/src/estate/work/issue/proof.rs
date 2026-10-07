use super::super::{Estate, Proof};
use super::{IssueWorktree, issue_stale};
use crate::{Error, Result, git};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueProving {
    pub issue: super::super::super::Coordinate,
    pub revision: i64,
}

impl Estate {
    pub async fn prove_issue(&self, request: &IssueProving) -> Result<IssueWorktree> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let member = self.issue_member(&request.issue).await?;
        let source = self.issue_source(&member)?;
        let path = self.issue_path(&anchor)?;
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
            write: &member.claims,
        })
        .map_err(|error| Error::typed("concord.boundary.refused", error.to_string()))?;
        if !report.ok {
            return Err(Error::typed(
                "concord.boundary.outside",
                format!("changes outside claim: {}", report.outside.join(", ")),
            ));
        }
        let proof = Proof {
            key: 0,
            schema: plumb::boundary::SCHEMA.to_string(),
            plumb: crate::PLUMB.to_string(),
            base,
            head,
            claim: crate::claim::digest(&member.claims),
        };
        self.keep_issue_boundary(anchor.key, member.key, anchor.revision + 1, &proof)
            .await?;
        self.issue_member(&request.issue).await
    }

    async fn keep_issue_boundary(
        &self,
        anchor: i64,
        member: i64,
        revision: i64,
        proof: &Proof,
    ) -> Result<()> {
        let held = self
            .core
            .live("IssueBoundary")
            .await
            .map_err(super::super::fault)?
            .into_iter()
            .find(|row| row.int("member") == Some(member));
        let parent = member.to_string();
        let root = anchor.to_string();
        let next = revision.to_string();
        self.core
            .batch(async |tx| {
                if let Some(held) = &held {
                    tx.end("IssueBoundary", held.key()).await?;
                }
                tx.put(
                    "IssueBoundary",
                    &[
                        ("schema", proof.schema.as_str()),
                        ("plumb", proof.plumb.as_str()),
                        ("base", proof.base.as_str()),
                        ("head", proof.head.as_str()),
                        ("claim", proof.claim.as_str()),
                        ("member", parent.as_str()),
                        ("anchor", root.as_str()),
                    ],
                )
                .await?;
                tx.set("Anchor", anchor, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(super::super::fault)
    }
}

pub(super) mod delivery {
    use super::super::super::integration::{self, TRACKING};
    use super::super::delivery::{Authority, Member, member};
    use super::super::{IssueRelease, issue_stale};
    use crate::estate::{Estate, Guard, Proof};
    use crate::{Error, Reference, Result, git};
    use plumb::integration::{Expectation, Relation};
    use serde::Serialize;
    use std::path::PathBuf;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Settlement {
        pub authority: Authority,
        pub issue: crate::estate::Coordinate,
        pub prepared: i64,
        pub revision: i64,
        pub member: Member,
        pub boundary: Proof,
        pub reference: Reference,
        pub base: String,
        pub candidate: String,
        pub merge: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
    pub struct Completion {
        pub revision: i64,
        pub head: String,
        pub released: bool,
    }

    impl Estate {
        pub async fn complete(&self, guard: &Guard, request: &Settlement) -> Result<Completion> {
            self.agreed().await?;
            let anchor = self.issue(&request.issue).await?;
            issue_stale(anchor.revision, request.revision)?;
            let held = self.integration(&request.issue).await?;
            if held != request.member.integration || held != guard.integration {
                return Err(disagreement());
            }
            if request.base != held.branch {
                return Err(disagreement());
            }
            let active = match self.issue_member(&request.issue).await {
                Ok(held_member) => {
                    if member(&held_member) != request.member {
                        return Err(stale());
                    }
                    if held_member.proof.as_ref() != Some(&request.boundary) {
                        return Err(stale());
                    }
                    let references = self.issue_references(held_member.key, anchor.key).await?;
                    if references.as_slice() != [request.reference.clone()] {
                        return Err(Error::typed(
                            "concord.delivery.reference",
                            "Member pull coordinate changed before completion",
                        ));
                    }
                    true
                }
                Err(error) if error.code() == "concord.member.absent" => {
                    if !matches!(request.revision - request.prepared, 1 | 2) {
                        return Err(stale());
                    }
                    false
                }
                Err(error) => return Err(error),
            };
            let current = self.integration(&request.issue).await?;
            if current != held {
                return Err(Error::typed(
                    "concord.integration.changed",
                    "registered Integration changed during delivery completion",
                ));
            }
            let source = PathBuf::from(&held.path);
            let remote = integration::registration::origin(&source)?;
            if remote != held.repository {
                return Err(Error::typed(
                    "concord.integration.repository",
                    "Integration origin no longer matches its registered repository",
                ));
            }
            let before = git::at(&source).head()?;
            let target = git::at(&source).fetch(&held.remote, &held.branch)?;
            if !git::at(&source).ancestor(&request.merge, &target)? {
                return Err(Error::typed(
                    "concord.delivery.merge",
                    format!(
                        "observed merge {} is not reachable from fetched {}/{} head {target}",
                        request.merge, held.remote, held.branch
                    ),
                ));
            }
            request
                .authority
                .validate::<super::super::authority::Plumb>(&held)?;
            match &request.authority {
                Authority::Plumb { .. } => {
                    plumb::delivery::landed(&source, &request.candidate, &request.merge)
                }
                Authority::WharfNative { .. } => {
                    plumb::delivery::native::landed(&source, &request.candidate, &request.merge)
                }
            }
            .map_err(|error| Error::typed("concord.delivery.landed", error.message))?;
            let expected = Expectation::new(&held.branch, TRACKING, &target);
            let advanced = plumb::integration::advance(&source, &expected, &before)
                .map_err(|error| Error::typed("concord.integration.advance", error.to_string()))?;
            if advanced.relation != Relation::Equal || advanced.checkout.head != target {
                return Err(Error::typed(
                    "concord.integration.advance",
                    "integration checkout did not equal fetched origin/main",
                ));
            }
            let revision = if active {
                self.release_issue(&IssueRelease {
                    issue: request.issue.clone(),
                    revision: request.revision,
                })
                .await?
            } else {
                request.revision
            };
            Ok(Completion {
                revision,
                head: target,
                released: active,
            })
        }
    }

    fn disagreement() -> Error {
        Error::typed(
            "concord.delivery.integration",
            "registered Integration or delivery base changed",
        )
    }

    fn stale() -> Error {
        Error::typed(
            "concord.delivery.stale",
            "Member, Boundary, or released revision changed before completion",
        )
    }
}
