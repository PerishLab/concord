use super::issue::IssueWorktree;
use super::{Estate, Integration};
use crate::path::at;
use crate::{Error, Result, git};
use plumb::integration::{Inspection, Worktree};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) struct Inventory<'a> {
    pub inspection: &'a Inspection,
    pub members: &'a [IssueWorktree],
    pub integration: &'a Integration,
    pub pending: &'a Path,
}

pub(super) struct Provision<'a> {
    pub source: &'a Path,
    pub path: &'a Path,
    pub branch: &'a str,
    pub target: &'a str,
    pub inspection: &'a Inspection,
}

impl Estate {
    pub(super) fn inventory(&self, request: Inventory<'_>) -> Result<()> {
        let mut expected = BTreeMap::new();
        expected.insert(
            request.inspection.checkout.path.clone(),
            request.inspection.checkout.head.clone(),
        );
        for member in request
            .members
            .iter()
            .filter(|member| member.integration.key == request.integration.key)
        {
            expected.insert(
                self.space
                    .join(".issues")
                    .join(&member.node)
                    .join("worktree"),
                member.base.clone(),
            );
        }
        for worktree in request.inspection.worktrees.iter().filter(|worktree| {
            !worktree.staging
                && worktree
                    .branch
                    .as_deref()
                    .and_then(crate::automation::branch)
                    .is_none()
        }) {
            let path = worktree
                .path
                .canonicalize()
                .unwrap_or_else(|_| worktree.path.clone());
            let pending = request
                .pending
                .canonicalize()
                .unwrap_or_else(|_| request.pending.to_path_buf());
            if path == pending {
                continue;
            }
            if !expected.contains_key(&path) {
                return Err(Error::typed(
                    "concord.integration.worktree_unknown",
                    format!(
                        "unmanaged worktree {} is at head {}",
                        path.display(),
                        worktree.head.as_deref().unwrap_or("unknown")
                    ),
                ));
            }
        }
        for path in expected.keys() {
            if !request.inspection.worktrees.iter().any(|worktree| {
                worktree.path.canonicalize().ok().as_ref() == path.canonicalize().ok().as_ref()
            }) {
                return Err(Error::typed(
                    "concord.integration.worktree_missing",
                    format!("registered worktree is missing: {}", path.display()),
                ));
            }
        }
        Ok(())
    }
}

impl Provision<'_> {
    pub(super) fn apply(&self) -> Result<()> {
        if let Some(found) = find(self.inspection, self.path) {
            if !self.matches(found)? {
                return Err(Error::typed(
                    "concord.member.pending_disagreement",
                    format!(
                        "existing worktree does not match start intent: {}",
                        self.path.display()
                    ),
                ));
            }
            return Ok(());
        }
        if self.path.exists() || git::at(self.source).exists(self.branch)? {
            return Err(Error::typed(
                "concord.member.pending_disagreement",
                "derived Member path or branch exists without the exact resumable worktree",
            ));
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::new("member path has no Issue parent"))?;
        at(parent).directory()?;
        git::at(self.source).add(self.path, self.branch, false)?;
        if git::at(self.path).head()? != self.target {
            return Err(Error::typed(
                "concord.member.pending_disagreement",
                "created Member did not start at the exact synchronized baseline",
            ));
        }
        Ok(())
    }

    fn matches(&self, found: &Worktree) -> Result<bool> {
        if found.branch.as_deref() != Some(self.branch)
            || found.head.as_deref() != Some(self.target)
        {
            return Ok(false);
        }
        if found.bare || found.detached || found.locked {
            return Ok(false);
        }
        if found.prunable {
            return Ok(false);
        }
        Ok(
            git::at(self.path).identity()? == self.inspection.checkout.common
                && git::at(self.path).clean()?,
        )
    }
}

fn find<'a>(inspection: &'a Inspection, wanted: &Path) -> Option<&'a Worktree> {
    let wanted = wanted.canonicalize().ok()?;
    inspection
        .worktrees
        .iter()
        .find(|worktree| worktree.path.canonicalize().ok().as_ref() == Some(&wanted))
}
