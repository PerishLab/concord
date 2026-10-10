use super::cleanup::Cleanup;
use super::{Estate, issue_stale};
use crate::{Coordinate, Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recovery {
    pub issue: Coordinate,
    pub revision: i64,
    pub head: String,
}

impl Estate {
    pub async fn recover(&self, request: &Recovery) -> Result<i64> {
        let _guard = self.guard()?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let member = self.issue_member(&request.issue).await?;
        if member.integration.repository.owner != request.issue.owner
            || member.integration.repository.name != request.issue.repository
        {
            return Err(Error::typed(
                "concord.member.recovery",
                "Member Integration coordinate disagrees",
            ));
        }
        let path = self.issue_path(&anchor)?;
        let source = self.issue_source(&member)?;
        let expected = member
            .proof
            .as_ref()
            .map_or(&member.base, |proof| &proof.head);
        if request.head != *expected {
            return Err(Error::typed(
                "concord.member.recovery",
                "head must equal the retained Boundary head or unchanged Member base",
            ));
        }
        let report = self.inspect().await?;
        let allowed = report.faults.iter().all(|fault| match fault.code.as_str() {
            "member.agreement" | "member.missing" => fault.subject == request.issue.identity(),
            "integration.worktree.missing" => fault.subject == path.display().to_string(),
            _ => false,
        });
        if !allowed {
            return Err(Error::detailed(
                "concord.audit.refused",
                "recovery refuses unrelated agreement faults",
                serde_json::json!({"agreement": report}),
            ));
        }
        let root = path.parent().expect("derived Issue path");
        if root.exists() && std::fs::symlink_metadata(root)?.file_type().is_symlink() {
            return Err(Error::typed(
                "concord.member.recovery",
                "Issue seat is a symlink",
            ));
        }
        Cleanup {
            source: &source,
            path: &path,
            branch: &member.branch,
            head: &request.head,
        }
        .restore()?;
        self.ensure().await?;
        Ok(anchor.revision)
    }
}
