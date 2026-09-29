use super::client::Client;
use super::git::Git;
use concord_core::{Error, Reference, ReferenceKind, Result};
use plumb::landing::Preparation;
use std::path::Path;

#[cfg(test)]
pub use super::model::Commit;
pub use super::model::{Pull, Report, State};

pub async fn fetch(root: &Path) -> Result<()> {
    Git::fetch(root).await
}

pub struct Access<'a> {
    pub identity: &'a Path,
    pub repository: &'a str,
    pub command: &'a Path,
    pub timeout: u64,
}

pub struct Service<'a> {
    preparation: &'a Preparation,
    client: Client<'a>,
}

impl<'a> Service<'a> {
    pub async fn open(preparation: &'a Preparation, access: Access<'a>) -> Result<Self> {
        let observed = Git::repository(access.identity).await?;
        if observed != access.repository {
            return Err(stale(format!(
                "delivery repository {} is not worktree origin {observed}",
                access.repository
            )));
        }
        Ok(Self {
            preparation,
            client: Client {
                command: access.command,
                repository: access.repository,
                timeout: access.timeout,
            },
        })
    }

    pub async fn locate(&mut self, reference: Option<&Reference>) -> Result<Option<Pull>> {
        let pull = if let Some(reference) = reference {
            self.reference(reference)?;
            Some(self.client.view(reference.number).await?)
        } else {
            let mut pulls = self
                .client
                .all(&self.preparation.base, &self.preparation.projection)
                .await?;
            if pulls.len() > 1 {
                return Err(stale("more than one pull matches the exact projection"));
            }
            pulls.pop()
        };
        pull.map(|pull| self.validate(pull)).transpose()
    }

    pub async fn publish(&self) -> Result<()> {
        let root = &self.preparation.root;
        let branches = [
            (&self.preparation.branch, &self.preparation.source),
            (&self.preparation.projection, &self.preparation.candidate),
        ];
        let mut arguments = vec!["push".to_string()];
        for (branch, _) in branches {
            let reference = format!("refs/heads/{branch}");
            let tracking = format!("refs/remotes/origin/{branch}");
            let expected = Git::optional(root, &tracking).await?;
            arguments.push(format!("--force-with-lease={reference}:{expected}"));
        }
        arguments.push("origin".to_string());
        for (branch, source) in branches {
            arguments.push(format!("{source}:refs/heads/{branch}"));
        }
        let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
        Git::run(root, &arguments).await.map(|_| ())
    }

    pub async fn raise(&mut self) -> Result<Pull> {
        let pull = self
            .client
            .raise(
                &self.preparation.base,
                &self.preparation.projection,
                &self.preparation.title,
                &self.preparation.body,
            )
            .await?;
        self.validate(pull)
    }

    pub async fn view(&mut self, number: i64) -> Result<Pull> {
        let pull = self.client.view(number).await?;
        self.validate(pull)
    }

    pub async fn mark(&mut self) -> Result<()> {
        self.client.mark(&self.preparation.candidate).await
    }

    pub async fn settle(&mut self, number: i64, squash: &plumb::delivery::Squash) -> Result<()> {
        self.client.settle(number, squash).await
    }

    pub fn report(&self, pull: Pull) -> Report {
        Report {
            pull,
            candidate: self.preparation.candidate.clone(),
        }
    }

    fn validate(&self, pull: Pull) -> Result<Pull> {
        super::model::contract(&pull, self.preparation, self.client.repository)?;
        Ok(pull)
    }

    fn reference(&self, reference: &Reference) -> Result<()> {
        if reference.kind != ReferenceKind::Change {
            return Err(stale("recorded coordinate is not a pull change"));
        }
        if reference.provider != "github" {
            return Err(stale("recorded pull provider is not GitHub"));
        }
        if format!("{}/{}", reference.owner, reference.repository) != self.client.repository {
            return Err(stale(
                "recorded pull coordinate does not match the delivery repository",
            ));
        }
        Ok(())
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

pub(super) fn stale(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.stale", message)
}

pub(super) fn provider(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.provider", message)
}
