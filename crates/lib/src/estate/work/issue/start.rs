use super::super::Estate;
use super::super::integration::{self, TRACKING};
use super::super::provision::{Inventory, Provision};
use super::{IssueMemberChange, IssueWorktree, branch, issue_stale};
use crate::{Error, Result, component, git};
use plumb::integration::{Expectation, Relation};
use std::path::{Path, PathBuf};

struct Replay<'a> {
    request: &'a Start,
    anchor: &'a super::super::super::Anchor,
    member: &'a IssueWorktree,
    claims: &'a [String],
    path: &'a Path,
    target: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Start {
    pub issue: super::super::super::Coordinate,
    pub node: String,
    pub stable: String,
    pub claims: Vec<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn start(&self, request: &Start) -> Result<IssueMemberChange> {
        component("Issue node", &request.node)?;
        component("repository node", &request.stable)?;
        let claims = crate::claim::normalize(&request.claims)?;
        let anchor = self.issue(&request.issue).await?;
        observed(&anchor.node, &request.node, "Issue")?;
        let integration = self.integration(&anchor.coordinate).await?;
        observed(&integration.node, &request.stable, "repository")?;
        let path = self.issue_path(&anchor)?;
        self.allow(&path).await?;
        let _integration = integration::guard(self, &integration)?;
        let source = PathBuf::from(&integration.path);
        let remote = integration::registration::origin(&source)?;
        if remote != integration.repository {
            return Err(Error::typed(
                "concord.integration.repository",
                format!(
                    "origin repository {} does not match registered {}",
                    remote.identity(),
                    integration.repository.identity()
                ),
            ));
        }
        let before = git::at(&source).head()?;
        let target = git::at(&source).fetch(&integration.remote, &integration.branch)?;
        let expectation = Expectation::new(&integration.branch, TRACKING, &target);
        let inspection = plumb::integration::advance(&source, &expectation, &before)
            .map_err(|error| Error::typed("concord.integration.advance", error.to_string()))?;
        if inspection.relation != Relation::Equal || inspection.checkout.head != target {
            return Err(Error::typed(
                "concord.integration.advance",
                "integration checkout did not equal fetched origin/main",
            ));
        }
        let _estate = self.guard()?;
        self.allow(&path).await?;
        let anchor = self.issue(&request.issue).await?;
        observed(&anchor.node, &request.node, "Issue")?;
        let held = self.integration(&anchor.coordinate).await?;
        if held != integration {
            return Err(Error::typed(
                "concord.integration.changed",
                "registered Integration changed while Issue start was in progress",
            ));
        }
        let members = self.issue_worktrees().await?;
        self.inventory(Inventory {
            inspection: &inspection,
            members: &members,
            integration: &integration,
            pending: &path,
        })?;
        if let Some(member) = members.iter().find(|member| member.node == anchor.node) {
            return self
                .resume(Replay {
                    request,
                    anchor: &anchor,
                    member,
                    claims: &claims,
                    path: &path,
                    target: &target,
                })
                .await;
        }
        issue_stale(anchor.revision, request.revision)?;
        let name = branch(&anchor.node);
        Provision {
            source: &source,
            path: &path,
            branch: &name,
            target: &target,
            inspection: &inspection,
        }
        .apply()?;
        let observations = self.issue_overlaps(None, &source, &claims).await?;
        let revision = anchor.revision + 1;
        let next = revision.to_string();
        let root = anchor.key.to_string();
        let key = integration.key.to_string();
        self.core
            .batch(async |tx| {
                let member = tx
                    .put(
                        "IssueMember",
                        &[
                            ("branch", name.as_str()),
                            ("base", target.as_str()),
                            ("anchor", root.as_str()),
                            ("integration", key.as_str()),
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
                Ok(())
            })
            .await
            .map_err(|error| {
                Error::typed(
                    "concord.member.pending",
                    format!(
                        "{}; exact worktree remains resumable at {}",
                        error,
                        path.display()
                    ),
                )
            })?;
        Ok(IssueMemberChange {
            member: self.issue_member(&request.issue).await?,
            observations,
        })
    }

    async fn resume(&self, replay: Replay<'_>) -> Result<IssueMemberChange> {
        if replay.verify().is_err()
            || !self
                .issue_references(replay.member.key, replay.anchor.key)
                .await?
                .is_empty()
        {
            return Err(Error::typed(
                "concord.member.reserved",
                format!(
                    "existing Member does not match the exact start intent: {}",
                    replay.request.issue.identity()
                ),
            ));
        }
        Ok(IssueMemberChange {
            member: replay.member.clone(),
            observations: self
                .issue_overlaps(
                    Some(replay.member.key),
                    Path::new(&replay.member.integration.path),
                    replay.claims,
                )
                .await?,
        })
    }

    async fn allow(&self, pending: &Path) -> Result<()> {
        let report = self.inspect().await?;
        let subject = pending.display().to_string();
        let retained = report
            .faults
            .iter()
            .filter(|finding| {
                finding.subject != subject
                    || !matches!(
                        finding.code.as_str(),
                        "integration.worktree.unknown" | "territory.unknown"
                    )
            })
            .count();
        if retained == 0 {
            return Ok(());
        }
        Err(Error::detailed(
            "concord.audit.refused",
            format!("estate has {retained} unrelated agreement fault(s)"),
            serde_json::json!({"agreement": report}),
        ))
    }
}

impl Replay<'_> {
    fn verify(&self) -> Result<()> {
        if self.anchor.revision != self.request.revision + 1
            || self.member.integration.node != self.request.stable
        {
            return Err(Error::new("revision or repository changed"));
        }
        if self.member.branch != branch(&self.anchor.node) || self.member.base != self.target {
            return Err(Error::new("branch or baseline changed"));
        }
        if self.member.claims != self.claims || self.member.proof.is_some() {
            return Err(Error::new("claims or proof changed"));
        }
        if git::at(self.path).branch()?.as_str() != self.member.branch
            || git::at(self.path).head()? != self.target
        {
            return Err(Error::new("worktree identity changed"));
        }
        if !git::at(self.path).clean()? {
            return Err(Error::new("worktree changed"));
        }
        Ok(())
    }
}

fn observed(expected: &str, found: &str, kind: &str) -> Result<()> {
    if expected == found {
        return Ok(());
    }
    Err(Error::typed(
        "concord.member.observation",
        format!("{kind} node changed: expected {expected}, found {found}"),
    ))
}
