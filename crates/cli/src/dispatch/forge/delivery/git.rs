use super::pull::{answer, provider};
use concord_core::Result;
use std::path::Path;
use tokio::process::Command;

pub(crate) struct Git;

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

    #[locus::trace(with = plumb::observation::view())]
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
