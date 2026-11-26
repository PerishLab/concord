use super::pull::{answer, provider};
use concord_core::Result;
use std::collections::BTreeSet;
use std::path::Path;
use tokio::process::Command;

pub(crate) struct Git;

impl Git {
    pub async fn fetch(root: &Path) -> Result<()> {
        Self::run(root, &["fetch", "--prune", "origin"])
            .await
            .map(|_| ())
    }

    pub async fn sync(root: &Path, branches: &[&str]) -> Result<()> {
        let refs = branches
            .iter()
            .map(|branch| format!("refs/heads/{branch}"))
            .collect::<Vec<_>>();
        let mut args = vec!["ls-remote", "--heads", "origin"];
        args.extend(refs.iter().map(String::as_str));
        let listed = Self::run(root, &args).await?;
        let present = listed
            .lines()
            .filter_map(|line| line.split_once('\t')?.1.strip_prefix("refs/heads/"))
            .collect::<BTreeSet<_>>();
        for branch in branches {
            let tracking = format!("refs/remotes/origin/{branch}");
            if present.contains(branch) {
                let spec = format!("+refs/heads/{branch}:{tracking}");
                Self::run(root, &["fetch", "--no-tags", "origin", &spec]).await?;
            } else if !Self::optional(root, &tracking).await?.is_empty() {
                Self::run(root, &["update-ref", "-d", &tracking]).await?;
            }
        }
        Ok(())
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
