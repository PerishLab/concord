use crate::path::at;
use crate::{Error, Result, git};
use plumb::integration::{Inspection, Worktree};
use std::path::Path;

pub(super) struct Provision<'a> {
    pub source: &'a Path,
    pub path: &'a Path,
    pub branch: &'a str,
    pub target: &'a str,
    pub inspection: &'a Inspection,
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
