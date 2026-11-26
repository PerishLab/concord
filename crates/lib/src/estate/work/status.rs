use crate::{Result, git};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Boundary {
    Absent,
    Current,
    Stale,
}

impl Boundary {
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
pub enum Relation {
    Reachable,
    #[serde(rename = "tree-equivalent")]
    Equivalent,
    Unlanded,
}

impl Relation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Reachable => "reachable",
            Self::Equivalent => "tree-equivalent",
            Self::Unlanded => "unlanded",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Checkout {
    pub path: String,
    pub head: String,
    #[serde(rename = "tracked_changes")]
    pub tracked: usize,
    #[serde(rename = "untracked_files")]
    pub untracked: usize,
    pub clean: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Upstream {
    pub reference: String,
    pub head: String,
    pub ahead: usize,
    pub behind: usize,
}

pub(super) fn checkout(path: &std::path::Path) -> Result<Checkout> {
    let held = git::at(path);
    let head = held.head()?;
    let (tracked, untracked) = held.changes()?;
    Ok(Checkout {
        path: path.display().to_string(),
        head,
        tracked,
        untracked,
        clean: tracked == 0 && untracked == 0,
    })
}

pub(super) fn integration(
    path: &std::path::Path,
    source: &std::path::Path,
    member: &Checkout,
    integration: &Checkout,
) -> Result<Relation> {
    if git::at(source).ancestor(&member.head, &integration.head)? {
        return Ok(Relation::Reachable);
    }
    let left = git::at(path).tree(&member.head)?;
    let right = git::at(source).tree(&integration.head)?;
    if left == right {
        return Ok(Relation::Equivalent);
    }
    Ok(Relation::Unlanded)
}

pub(super) fn upstream(path: &std::path::Path, branch: &str) -> Result<Option<Upstream>> {
    let held = git::at(path);
    let Some(reference) = held.upstream(branch)? else {
        return Ok(None);
    };
    let head = held.text(&["rev-parse", &reference])?;
    let (ahead, behind) = held.divergence("HEAD", &reference)?;
    Ok(Some(Upstream {
        reference,
        head,
        ahead,
        behind,
    }))
}
