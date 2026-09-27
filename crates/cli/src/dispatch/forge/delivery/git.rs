use super::pull::{answer, provider};
use concord_core::Result;
use std::path::Path;
use tokio::process::Command;

pub(super) struct Git;

impl Git {
    pub async fn fetch(root: &Path) -> Result<()> {
        Self::run(root, &["fetch", "--prune", "origin"])
            .await
            .map(|_| ())
    }

    pub async fn optional(root: &Path, reference: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--verify", reference])
            .current_dir(root)
            .output()
            .await
            .map_err(|error| provider(format!("cannot run git: {error}")))?;
        Ok(if output.status.success() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            String::new()
        })
    }

    pub async fn repository(root: &Path) -> Result<String> {
        let remote = Self::run(root, &["remote", "get-url", "origin"]).await?;
        let path = remote
            .strip_prefix("git@github.com:")
            .or_else(|| remote.strip_prefix("ssh://git@github.com/"))
            .or_else(|| remote.strip_prefix("https://github.com/"))
            .ok_or_else(|| provider("origin is not GitHub"))?
            .trim_end_matches(".git");
        let (owner, repository) = path
            .split_once('/')
            .filter(|(owner, repository)| {
                !owner.is_empty() && !repository.is_empty() && !repository.contains('/')
            })
            .ok_or_else(|| provider("cannot derive owner/repository from origin"))?;
        Ok(format!("{owner}/{repository}"))
    }

    pub async fn run(root: &Path, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .await
            .map_err(|error| provider(format!("cannot run git: {error}")))?;
        answer(output, "git")
    }
}
