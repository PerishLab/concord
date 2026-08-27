use super::{Estate, Worktree};
use crate::{Result, git};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaimOverlap {
    pub code: String,
    pub peer: String,
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemberChange {
    pub member: Worktree,
    pub observations: Vec<ClaimOverlap>,
}

impl Estate {
    pub(super) async fn overlaps(
        &self,
        owner: Option<i64>,
        source: &Path,
        claims: &[String],
    ) -> Result<Vec<ClaimOverlap>> {
        let identity = git::at(source).identity()?;
        let mut observations = Vec::new();
        for member in self.worktrees().await? {
            if owner == Some(member.key) {
                continue;
            }
            let held = self.source(&member)?;
            if git::at(&held).identity()? == identity {
                let paths = crate::claim::intersections(claims, &member.claims);
                if !paths.is_empty() {
                    observations.push(ClaimOverlap {
                        code: "claim.overlap".to_string(),
                        peer: format!("{}/{}", member.task, member.name),
                        paths,
                    });
                }
            }
        }
        observations.sort_by_key(|observation| observation.peer.clone());
        Ok(observations)
    }
}
