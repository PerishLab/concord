use super::Proof;
use crate::{Result, git};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaimOverlap {
    pub code: String,
    pub peer: String,
    pub paths: Vec<String>,
    pub committed: CommittedOverlap,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum CommittedOverlap {
    DeclaredOnly,
    Intersecting {
        paths: Vec<String>,
    },
    Unavailable {
        member: String,
        reason: CommittedEvidence,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommittedEvidence {
    Dirty,
    Stale,
    MissingBase,
    Unavailable,
}

pub(super) struct OverlapMember {
    pub(super) key: Option<i64>,
    pub(super) identity: String,
    pub(super) path: PathBuf,
    pub(super) base: String,
    pub(super) claims: Vec<String>,
    pub(super) proof: Option<Proof>,
}

pub(super) fn same_repository(left: &Path, right: &Path) -> Result<bool> {
    Ok(git::at(left).identity()? == git::at(right).identity()?)
}

pub(super) fn committed_overlap(left: &OverlapMember, right: &OverlapMember) -> CommittedOverlap {
    let left = match committed_delta(left) {
        Ok(paths) => paths,
        Err(reason) => {
            return CommittedOverlap::Unavailable {
                member: left.identity.clone(),
                reason,
            };
        }
    };
    let right = match committed_delta(right) {
        Ok(paths) => paths,
        Err(reason) => {
            return CommittedOverlap::Unavailable {
                member: right.identity.clone(),
                reason,
            };
        }
    };
    let right = right.into_iter().collect::<BTreeSet<_>>();
    let paths = left
        .into_iter()
        .filter(|path| right.contains(path))
        .collect::<Vec<_>>();
    if paths.is_empty() {
        CommittedOverlap::DeclaredOnly
    } else {
        CommittedOverlap::Intersecting { paths }
    }
}

fn committed_delta(member: &OverlapMember) -> std::result::Result<Vec<String>, CommittedEvidence> {
    let checkout = git::at(&member.path);
    match checkout.clean() {
        Ok(true) => {}
        Ok(false) => return Err(CommittedEvidence::Dirty),
        Err(_) => return Err(CommittedEvidence::Unavailable),
    }
    let head = checkout
        .head()
        .map_err(|_| CommittedEvidence::Unavailable)?;
    let base = if let Some(proof) = &member.proof {
        if !proof.current(&head, &crate::claim::digest(&member.claims)) {
            return Err(CommittedEvidence::Stale);
        }
        proof.base.as_str()
    } else {
        member.base.as_str()
    };
    if !checkout
        .commit_exists(base)
        .map_err(|_| CommittedEvidence::Unavailable)?
    {
        return Err(CommittedEvidence::MissingBase);
    }
    if !checkout
        .ancestor(base, &head)
        .map_err(|_| CommittedEvidence::Unavailable)?
    {
        return Err(CommittedEvidence::Stale);
    }
    checkout
        .changed_paths(base, &head)
        .map_err(|_| CommittedEvidence::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn missing() {
        let temp = tempfile::tempdir().expect("temporary repository");
        let root = temp.path();
        run(root, &["init", "-b", "main"]);
        run(root, &["config", "user.name", "Concord Test"]);
        run(root, &["config", "user.email", "concord@example.invalid"]);
        std::fs::write(root.join("README.md"), "fixture\n").expect("fixture");
        run(root, &["add", "README.md"]);
        run(root, &["commit", "-m", "fixture"]);
        let base = git::at(root).head().expect("base");
        let left = member(root, "0000000000000000000000000000000000000000");
        let right = member(root, &base);
        assert_eq!(
            committed_overlap(&left, &right),
            CommittedOverlap::Unavailable {
                member: "member".to_string(),
                reason: CommittedEvidence::MissingBase,
            }
        );
    }

    #[test]
    fn absent() {
        let temp = tempfile::tempdir().expect("temporary repository");
        let root = temp.path();
        run(root, &["init", "-b", "main"]);
        run(root, &["config", "user.name", "Concord Test"]);
        run(root, &["config", "user.email", "concord@example.invalid"]);
        std::fs::write(root.join("README.md"), "fixture\n").expect("fixture");
        run(root, &["add", "README.md"]);
        run(root, &["commit", "-m", "fixture"]);
        let base = git::at(root).head().expect("base");
        let left = member(&root.join("absent"), &base);
        let right = member(root, &base);
        assert_eq!(
            committed_overlap(&left, &right),
            CommittedOverlap::Unavailable {
                member: "member".to_string(),
                reason: CommittedEvidence::Unavailable,
            }
        );
    }

    fn member(root: &Path, base: &str) -> OverlapMember {
        OverlapMember {
            key: None,
            identity: "member".to_string(),
            path: root.to_path_buf(),
            base: base.to_string(),
            claims: vec![".".to_string()],
            proof: None,
        }
    }

    fn run(root: &Path, arguments: &[&str]) {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(arguments)
                .status()
                .expect("run Git")
                .success()
        );
    }
}
