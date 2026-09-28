use crate::{Result, git};
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

pub(super) fn checkout(path: &std::path::Path) -> Result<CheckoutState> {
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

pub(super) fn integration(
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

pub(super) fn upstream(path: &std::path::Path, branch: &str) -> Result<Option<UpstreamState>> {
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
