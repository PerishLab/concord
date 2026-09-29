use super::git::Git;
use concord_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

const FIELDS: &str = "id,number,url,headRefOid,title,body";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pull {
    pub id: String,
    pub number: i64,
    pub url: String,
    #[serde(rename = "headRefOid")]
    pub head: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Report {
    pub pull: Pull,
    pub candidate: String,
    pub merged: bool,
}

pub async fn fetch(root: &Path) -> Result<()> {
    Git::fetch(root).await
}

pub async fn land(
    preparation: &plumb::landing::Preparation,
    expected_repository: &str,
    command: &Path,
    timeout: u64,
) -> Result<Report> {
    let root = &preparation.root;
    let repository = Git::repository(root).await?;
    if repository != expected_repository {
        return Err(stale(format!(
            "delivery repository {expected_repository} is not worktree origin {repository}"
        )));
    }
    let mut client = Client {
        command,
        repository: &repository,
        timeout,
    };
    let standing = client
        .opened(&preparation.base, &preparation.projection)
        .await?;
    if standing
        .as_ref()
        .is_some_and(|pull| pull.title != preparation.title || pull.body != preparation.body)
    {
        return Err(stale("standing pull narrative changed"));
    }
    push(root, &preparation.branch, &preparation.source, true).await?;
    push(root, &preparation.projection, &preparation.candidate, false).await?;
    let pull = match standing {
        Some(pull) => pull,
        None => {
            client
                .raise(
                    &preparation.base,
                    &preparation.projection,
                    &preparation.title,
                    &preparation.body,
                )
                .await?
        }
    };
    let pull = client.view(&pull.url).await?;
    if pull.head != preparation.candidate
        || pull.title != preparation.title
        || pull.body != preparation.body
    {
        return Err(stale(
            "pull head or narrative does not match the exact delivery plan",
        ));
    }
    Ok(Report {
        pull,
        candidate: preparation.candidate.clone(),
        merged: false,
    })
}

async fn push(root: &Path, branch: &str, source: &str, upstream: bool) -> Result<()> {
    let reference = format!("refs/heads/{branch}");
    let tracking = format!("refs/remotes/origin/{branch}");
    let expected = Git::optional(root, &tracking).await?;
    let lease = format!("--force-with-lease={reference}:{expected}");
    let spec = format!("{source}:{reference}");
    let mut args = vec!["push"];
    if upstream {
        args.push("-u");
    }
    args.extend([lease.as_str(), "origin", spec.as_str()]);
    Git::run(root, &args).await.map(|_| ())
}

pub(super) struct Client<'a> {
    pub command: &'a Path,
    pub repository: &'a str,
    pub timeout: u64,
}

impl Client<'_> {
    async fn opened(&mut self, base: &str, head: &str) -> Result<Option<Pull>> {
        let body = self
            .gh(&[
                "pr",
                "list",
                "-R",
                self.repository,
                "--state",
                "open",
                "--base",
                base,
                "--head",
                head,
                "--json",
                FIELDS,
            ])
            .await?;
        serde_json::from_str::<Vec<Pull>>(&body)
            .map(|mut pulls| pulls.drain(..).next())
            .map_err(|error| provider(format!("gh pr list did not answer JSON: {error}")))
    }

    async fn raise(&mut self, base: &str, head: &str, title: &str, body: &str) -> Result<Pull> {
        let reply = self
            .gh(&[
                "pr",
                "create",
                "-R",
                self.repository,
                "--base",
                base,
                "--head",
                head,
                "--title",
                title,
                "--body",
                body,
            ])
            .await?;
        let url = reply
            .lines()
            .rev()
            .find(|line| line.contains("/pull/"))
            .ok_or_else(|| provider("gh pr create named no pull"))?;
        self.view(url).await
    }

    async fn view(&mut self, pull: &str) -> Result<Pull> {
        let body = self
            .gh(&["pr", "view", pull, "-R", self.repository, "--json", FIELDS])
            .await?;
        serde_json::from_str(&body)
            .map_err(|error| provider(format!("gh pr view did not answer JSON: {error}")))
    }

    pub async fn mark(&mut self, candidate: &str) -> Result<()> {
        self.gh(&[
            "api",
            "-X",
            "POST",
            &format!("repos/{}/statuses/{candidate}", self.repository),
            "-f",
            "state=success",
            "-f",
            "context=guard / guard (pull_request)",
            "-f",
            "description=Concord revalidated the exact Plumb Guard proof",
        ])
        .await
        .map(|_| ())
    }

    pub async fn settle(&mut self, number: i64, squash: &plumb::delivery::Squash) -> Result<()> {
        let number = u64::try_from(number)
            .map_err(|_| provider(format!("pull number {number} is negative")))?;
        let arguments = squash.arguments(self.repository, number);
        let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
        let mut last = None;
        for turn in 1..=6 {
            match self.gh(&arguments).await {
                Ok(_) => return Ok(()),
                Err(error) if pending(error.message()) => last = Some(error),
                Err(error) => return Err(error),
            }
            tokio::time::sleep(Duration::from_secs(turn)).await;
        }
        Err(last.unwrap_or_else(|| provider("pull did not become mergeable")))
    }

    async fn gh(&mut self, args: &[&str]) -> Result<String> {
        let output = tokio::time::timeout(
            Duration::from_secs(self.timeout),
            Command::new(self.command).args(args).output(),
        )
        .await
        .map_err(|_| provider("GitHub command timed out"))?
        .map_err(|error| provider(format!("cannot run GitHub command: {error}")))?;
        answer(output, "gh")
    }
}

pub(super) fn answer(output: std::process::Output, command: &str) -> Result<String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    Err(provider(format!(
        "{command} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn pending(message: &str) -> bool {
    ["not mergeable", "is expected", "mergeability"]
        .iter()
        .any(|needle| message.contains(needle))
}

pub(super) fn stale(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.stale", message)
}

pub(super) fn provider(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.provider", message)
}

#[cfg(test)]
mod tests {
    use super::{Pull, pending};

    #[test]
    fn contract() {
        let pull: Pull = serde_json::from_str(
            r#"{"id":"PR_node","number":28,"url":"https://github.com/PerishLab/concord/pull/28","headRefOid":"abc","title":"Deliver","body":"Refs PerishLab/concord#28"}"#,
        )
        .expect("provider Pull");
        assert_eq!(pull.id, "PR_node");
        assert_eq!(pull.head, "abc");
        assert!(pending("pull is not mergeable"));
        assert!(!pending("authentication failed"));
    }
}
