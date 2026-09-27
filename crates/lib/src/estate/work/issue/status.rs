use super::super::{BoundaryState, CheckoutState, Estate, IntegrationState, UpstreamState};
use super::IssueWorktree;
use crate::{PLUMB, Reference, Result, git};
use serde::Serialize;

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
        name: &str,
    ) -> Result<IssueMemberStatus> {
        self.executable()?;
        let anchor = self.issue(issue).await?;
        let member = self.issue_member(issue, name).await?;
        let references = self.issue_references(member.key, anchor.key).await?;
        let path = self.issue_path(&anchor, &member.name)?;
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
