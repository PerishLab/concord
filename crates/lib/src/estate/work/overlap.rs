use super::Proof;
use crate::{Result, git};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Overlap {
    pub code: String,
    pub peer: String,
    pub paths: Vec<String>,
    pub committed: Committed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Committed {
    #[serde(rename = "declared-only")]
    Declared,
    Intersecting {
        paths: Vec<String>,
    },
    Unavailable {
        member: String,
        reason: Evidence,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Evidence {
    Dirty,
    Stale,
    #[serde(rename = "missing-base")]
    Missing,
    Unavailable,
}

pub(super) struct Subject {
    pub(super) key: Option<i64>,
    pub(super) identity: String,
    pub(super) path: PathBuf,
    pub(super) base: String,
    pub(super) claims: Vec<String>,
    pub(super) proof: Option<Proof>,
}

pub(super) fn same(identities: &mut git::Identities, left: &Path, right: &Path) -> Result<bool> {
    Ok(identities.read(left)? == identities.read(right)?)
}

pub(super) fn committed(left: &Subject, right: &Subject) -> Committed {
    let left = match delta(left) {
        Ok(paths) => paths,
        Err(reason) => {
            return Committed::Unavailable {
                member: left.identity.clone(),
                reason,
            };
        }
    };
    let right = match delta(right) {
        Ok(paths) => paths,
        Err(reason) => {
            return Committed::Unavailable {
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
        Committed::Declared
    } else {
        Committed::Intersecting { paths }
    }
}

fn delta(member: &Subject) -> std::result::Result<Vec<String>, Evidence> {
    let checkout = git::at(&member.path);
    match checkout.clean() {
        Ok(true) => {}
        Ok(false) => return Err(Evidence::Dirty),
        Err(_) => return Err(Evidence::Unavailable),
    }
    let head = checkout.head().map_err(|_| Evidence::Unavailable)?;
    let base = if let Some(proof) = &member.proof {
        if !proof.current(&head, &crate::claim::digest(&member.claims)) {
            return Err(Evidence::Stale);
        }
        proof.base.as_str()
    } else {
        member.base.as_str()
    };
    if !checkout.commit(base).map_err(|_| Evidence::Unavailable)? {
        return Err(Evidence::Missing);
    }
    if !checkout
        .ancestor(base, &head)
        .map_err(|_| Evidence::Unavailable)?
    {
        return Err(Evidence::Stale);
    }
    checkout
        .delta(base, &head)
        .map_err(|_| Evidence::Unavailable)
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
            committed(&left, &right),
            Committed::Unavailable {
                member: "member".to_string(),
                reason: Evidence::Missing,
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
            committed(&left, &right),
            Committed::Unavailable {
                member: "member".to_string(),
                reason: Evidence::Unavailable,
            }
        );
    }

    fn member(root: &Path, base: &str) -> Subject {
        Subject {
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
