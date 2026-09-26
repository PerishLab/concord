use super::super::{Reference, World};
use super::{Estate, Worktree};
use crate::{PLUMB, Result, git};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoundaryState {
    Absent,
    Current,
    Stale,
}

impl BoundaryState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Current => "current",
            Self::Stale => "stale",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationState {
    Reachable,
    TreeEquivalent,
    Unlanded,
}

impl IntegrationState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Reachable => "reachable",
            Self::TreeEquivalent => "tree-equivalent",
            Self::Unlanded => "unlanded",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckoutState {
    pub path: String,
    pub head: String,
    pub tracked_changes: usize,
    pub untracked_files: usize,
    pub clean: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UpstreamState {
    pub reference: String,
    pub head: String,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemberStatus {
    pub member: Worktree,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<Reference>,
    pub worktree: CheckoutState,
    pub integration_checkout: CheckoutState,
    pub boundary: BoundaryState,
    pub integration: IntegrationState,
    pub local_upstream: Option<UpstreamState>,
    pub local_tracking_refs: Vec<String>,
}

impl Estate {
    pub async fn member_status(&self, task: &str, name: &str) -> Result<MemberStatus> {
        let world = World::load(self).await?;
        let task = world.node(task)?;
        let identity = task.identity();
        let member = self.member(&identity, name).await?;
        let reference = self.forge(task.key, Some(member.key)).await?;
        let path = self.path(&task.domain, &task.name, &member.name);
        let source = self.source(&member)?;
        let worktree = checkout(&path)?;
        let integration_checkout = checkout(&source)?;
        let boundary = boundary(&member, &worktree.head);
        let integration = integration(&path, &source, &worktree, &integration_checkout)?;
        let local_upstream = upstream(&path, &member.branch)?;
        let local_tracking_refs = git::at(&path).tracking(&worktree.head)?;
        Ok(MemberStatus {
            member,
            reference,
            worktree,
            integration_checkout,
            boundary,
            integration,
            local_upstream,
            local_tracking_refs,
        })
    }
}

fn checkout(path: &std::path::Path) -> Result<CheckoutState> {
    let held = git::at(path);
    let head = held.head()?;
    let (tracked_changes, untracked_files) = held.changes()?;
    Ok(CheckoutState {
        path: path.display().to_string(),
        head,
        tracked_changes,
        untracked_files,
        clean: tracked_changes == 0 && untracked_files == 0,
    })
}

fn boundary(member: &Worktree, head: &str) -> BoundaryState {
    let Some(proof) = &member.proof else {
        return BoundaryState::Absent;
    };
    if current(proof, member, head) {
        BoundaryState::Current
    } else {
        BoundaryState::Stale
    }
}

fn current(proof: &super::Proof, member: &Worktree, head: &str) -> bool {
    if proof.schema != plumb::boundary::SCHEMA || proof.plumb != PLUMB {
        return false;
    }
    if proof.head != head {
        return false;
    }
    proof.claim == crate::claim::digest(&member.claims)
}

fn integration(
    path: &std::path::Path,
    source: &std::path::Path,
    member: &CheckoutState,
    integration: &CheckoutState,
) -> Result<IntegrationState> {
    if git::at(source).ancestor(&member.head, &integration.head)? {
        return Ok(IntegrationState::Reachable);
    }
    let left = git::at(path).tree(&member.head)?;
    let right = git::at(source).tree(&integration.head)?;
    if left == right {
        return Ok(IntegrationState::TreeEquivalent);
    }
    Ok(IntegrationState::Unlanded)
}

fn upstream(path: &std::path::Path, branch: &str) -> Result<Option<UpstreamState>> {
    let held = git::at(path);
    let Some(reference) = held.upstream(branch)? else {
        return Ok(None);
    };
    let head = held.text(&["rev-parse", &reference])?;
    let (ahead, behind) = held.divergence("HEAD", &reference)?;
    Ok(Some(UpstreamState {
        reference,
        head,
        ahead,
        behind,
    }))
}
