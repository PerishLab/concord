use crate::git;
use crate::{Result, TaskRef};
use serde::Serialize;
use std::path::Path;

use super::Fault;

#[derive(Clone, Debug, Serialize)]
pub struct Preflight {
    pub target: String,
    pub members: Vec<MemberPreflight>,
    pub faults: Vec<Fault>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemberPreflight {
    pub name: String,
    pub path: String,
    pub source: String,
    pub expected_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof: Option<MemberProof>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemberProof {
    pub source_identity: String,
    pub member_identity: String,
    pub registered: bool,
    pub branch: String,
    pub clean: bool,
    pub landing: LandingProof,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "relation", rename_all = "kebab-case")]
pub enum LandingProof {
    Reachable {
        member_head: String,
        integration_head: String,
    },
    TreeEquivalent {
        member_head: String,
        integration_head: String,
        member_tree: String,
        integration_tree: String,
    },
}

impl Preflight {
    pub fn ok(&self) -> bool {
        self.faults.is_empty() && self.members.iter().all(|member| member.proof.is_some())
    }

    pub fn unproved(&self) -> usize {
        self.members
            .iter()
            .filter(|member| member.proof.is_none())
            .count()
    }
}

impl TaskRef {
    pub fn preflight(&self) -> Result<Preflight> {
        let audit = self.agreement()?;
        let mut preflight = Preflight {
            target: audit.target,
            members: Vec::new(),
            faults: audit.faults,
        };
        for member in &self.task().repo {
            preflight
                .members
                .push(member_preflight(&mut preflight.faults, self, member)?);
        }
        Ok(preflight)
    }
}

fn member_preflight(
    faults: &mut Vec<Fault>,
    task: &TaskRef,
    member: &crate::Member,
) -> Result<MemberPreflight> {
    let path = task.member_path(&member.name);
    let expected_branch = member.branch(&task.task().name).to_string();
    let mut held = MemberPreflight {
        name: member.name.clone(),
        path: path.display().to_string(),
        source: member.source.clone(),
        expected_branch: expected_branch.clone(),
        proof: None,
    };
    if !path.is_dir() {
        return Ok(held);
    }
    let source = match task.source(&member.source) {
        Ok(source) => source,
        Err(_) => return Ok(held),
    };
    held.source = source.display().to_string();
    let (source_identity, member_identity) = match (git::identity(&source), git::identity(&path)) {
        (Ok(source_identity), Ok(member_identity)) => (source_identity, member_identity),
        _ => return Ok(held),
    };
    let registered = match git::registered(&source, &path) {
        Ok(registered) => registered,
        Err(_) => return Ok(held),
    };
    let branch = match git::branch(&path) {
        Ok(branch) => branch,
        Err(_) => return Ok(held),
    };
    if source_identity != member_identity || !registered || branch != expected_branch {
        return Ok(held);
    }
    match git::clean(&path) {
        Ok(true) => {}
        Ok(false) => {
            fault(
                faults,
                "dirty",
                &path,
                "member has dirty or untracked files",
            );
            return Ok(held);
        }
        Err(error) => {
            fault(faults, "worktree", &path, error.to_string());
            return Ok(held);
        }
    }
    let landing = match git::landing(&path, &source) {
        Ok(Some(landing)) => landing,
        Ok(None) => {
            fault(
                faults,
                "reachability",
                &path,
                "member HEAD is neither reachable nor tree-equivalent to the integration checkout HEAD",
            );
            return Ok(held);
        }
        Err(error) => {
            fault(faults, "reachability", &path, error.to_string());
            return Ok(held);
        }
    };
    held.proof = Some(MemberProof {
        source_identity: source_identity.display().to_string(),
        member_identity: member_identity.display().to_string(),
        registered: true,
        branch,
        clean: true,
        landing: landing.into(),
    });
    Ok(held)
}

impl From<git::Landing> for LandingProof {
    fn from(landing: git::Landing) -> Self {
        match landing {
            git::Landing::Reachable {
                member_head,
                source_head,
            } => Self::Reachable {
                member_head,
                integration_head: source_head,
            },
            git::Landing::TreeEquivalent {
                member_head,
                source_head,
                member_tree,
                source_tree,
            } => Self::TreeEquivalent {
                member_head,
                integration_head: source_head,
                member_tree,
                integration_tree: source_tree,
            },
        }
    }
}

fn fault(faults: &mut Vec<Fault>, kind: &str, path: &Path, message: impl Into<String>) {
    faults.push(Fault {
        kind: kind.to_string(),
        path: path.display().to_string(),
        message: message.into(),
    });
}
