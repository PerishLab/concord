use super::super::{Estate, Proof};
use super::{IssueWorktree, issue_stale};
use crate::{Error, Result, git};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueProving {
    pub issue: super::super::super::Coordinate,
    pub member: String,
    pub revision: i64,
}

impl Estate {
    pub async fn prove_issue(&self, request: &IssueProving) -> Result<IssueWorktree> {
        self.executable()?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let member = self.issue_member(&request.issue, &request.member).await?;
        let source = self.issue_source(&member)?;
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
        self.issue_member(&request.issue, &request.member).await
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
