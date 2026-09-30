use super::model::Pull;
use super::pull::provider;
use concord_core::Result;
use std::path::Path;
use std::time::Duration;

const LIMIT: usize = 1024 * 1024;

const FIELDS: &str =
    "id,number,url,state,baseRefName,headRefOid,mergeCommit,mergedAt,updatedAt,title,body";

pub(super) struct Client<'a> {
    pub command: &'a Path,
    pub repository: &'a str,
    pub timeout: u64,
}

impl Client<'_> {
    pub async fn all(&mut self, base: &str, head: &str) -> Result<Vec<Pull>> {
        let body = self
            .gh(&[
                "pr",
                "list",
                "-R",
                self.repository,
                "--state",
                "all",
                "--base",
                base,
                "--head",
                head,
                "--json",
                FIELDS,
            ])
            .await?;
        serde_json::from_str::<Vec<Pull>>(&body)
            .map_err(|error| provider(format!("gh pr list did not answer JSON: {error}")))
    }

    pub async fn raise(&mut self, base: &str, head: &str, title: &str, body: &str) -> Result<Pull> {
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
        let number = url
            .rsplit_once('/')
            .and_then(|(_, number)| number.parse::<i64>().ok())
            .ok_or_else(|| provider("gh pr create returned a malformed pull URL"))?;
        self.view(number).await
    }

    pub async fn view(&mut self, pull: i64) -> Result<Pull> {
        let pull = pull.to_string();
        let body = self
            .gh(&["pr", "view", &pull, "-R", self.repository, "--json", FIELDS])
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
        let output =
            super::super::github::transport::Request::new(self.command, self.timeout, LIMIT)
                .args(args.iter().copied())
                .run()
                .await
                .map_err(|failure| provider(failure.to_string()))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

fn pending(message: &str) -> bool {
    ["not mergeable", "is expected", "mergeability"]
        .iter()
        .any(|needle| message.contains(needle))
}

#[cfg(test)]
mod tests {
    #[test]
    fn pending() {
        assert!(super::pending("pull is not mergeable"));
        assert!(!super::pending("authentication failed"));
    }
}
