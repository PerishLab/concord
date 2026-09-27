use super::super::Estate;
use super::{IssueWorktree, issue_stale};
use crate::{Error, Result, git};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueRelease {
    pub issue: super::super::super::Coordinate,
    pub member: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueRetirement {
    pub issue: super::super::super::Coordinate,
    pub member: String,
    pub artifacts: Vec<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn release_issue(&self, request: &IssueRelease) -> Result<i64> {
        self.finish_issue_member(&request.issue, &request.member, request.revision, None)
            .await
    }

    pub async fn retire_issue(&self, request: &IssueRetirement) -> Result<i64> {
        self.finish_issue_member(
            &request.issue,
            &request.member,
            request.revision,
            Some(&request.artifacts),
        )
        .await
    }

    async fn finish_issue_member(
        &self,
        issue: &super::super::super::Coordinate,
        name: &str,
        revision: i64,
        artifacts: Option<&[String]>,
    ) -> Result<i64> {
        self.executable()?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(issue).await?;
        issue_stale(anchor.revision, revision)?;
        let member = self.issue_member(issue, name).await?;
        let path = self.issue_path(&anchor, &member.name)?;
        let source = self.issue_source(&member)?;
        current(&member, &git::at(&path).head()?)?;
        if !git::at(&path).clean()? {
            return Err(Error::typed(
                "concord.member.dirty",
                "Member has dirty or untracked files",
            ));
        }
        if artifacts.is_none() && !git::at(&path).landed(&source)? {
            return Err(Error::typed(
                "concord.member.unlanded",
                "Member HEAD is neither reachable nor tree-equivalent to source HEAD",
            ));
        }
        if let Some(patterns) = artifacts {
            let held = self.issue_artifacts(issue).await?;
            if !held.iter().any(|artifact| {
                patterns
                    .iter()
                    .any(|pattern| pattern == "*" || pattern == &artifact.name)
            }) {
                return Err(Error::typed(
                    "concord.artifact.absent",
                    "no Artifact matches the given patterns",
                ));
            }
        }
        let claims = self
            .core
            .live("IssueClaim")
            .await
            .map_err(super::super::fault)?;
        let changes = self
            .core
            .live("IssueChange")
            .await
            .map_err(super::super::fault)?;
        let proof = member.proof.as_ref().expect("current checked proof");
        let revision = anchor.revision + 1;
        let next = revision.to_string();
        git::at(&source).remove(&path)?;
        let changed = self
            .core
            .batch(async |tx| {
                tx.end("IssueBoundary", proof.key).await?;
                for row in claims
                    .iter()
                    .filter(|row| row.int("member") == Some(member.key))
                {
                    tx.end("IssueClaim", row.key()).await?;
                }
                for row in changes
                    .iter()
                    .filter(|row| row.int("member") == Some(member.key))
                {
                    tx.end("IssueChange", row.key()).await?;
                }
                tx.end("IssueMember", member.key).await?;
                tx.set("Anchor", anchor.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await;
        if let Err(error) = changed {
            return Err(Error::typed(
                "concord.member.disagreement",
                format!("{error}; worktree is removed but estate still declares it"),
            ));
        }
        if let Some(root) = path.parent()
            && root
                .read_dir()
                .ok()
                .and_then(|mut entries| entries.next())
                .is_none()
        {
            let _ = std::fs::remove_dir(root);
        }
        Ok(revision)
    }
}

fn current(member: &IssueWorktree, head: &str) -> Result<()> {
    let proof = member.proof.as_ref().ok_or_else(|| {
        Error::typed(
            "concord.boundary.absent",
            "Member has no current Boundary proof",
        )
    })?;
    let held = (
        proof.schema.as_str(),
        proof.plumb.as_str(),
        proof.head.as_str(),
        proof.claim.as_str(),
    );
    let claim = crate::claim::digest(&member.claims);
    if held != (plumb::boundary::SCHEMA, crate::PLUMB, head, claim.as_str()) {
        return Err(Error::typed(
            "concord.boundary.stale",
            "Member Boundary proof is stale",
        ));
    }
    Ok(())
}
