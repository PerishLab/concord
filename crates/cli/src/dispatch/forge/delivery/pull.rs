use super::adaptor::Client;
pub(super) use super::adaptor::Merge;
use super::git::Git;
use concord_core::delivery::{Mode, Preparation};
use concord_core::{Error, Reference, Result};
use std::path::Path;

#[cfg(test)]
pub(super) use super::adaptor::Commit;
pub(super) use super::adaptor::{Pull, Report, State};

pub async fn fetch(root: &Path) -> Result<()> {
    Git::fetch(root).await
}

pub async fn sync(root: &Path, branches: &[&str]) -> Result<()> {
    Git::sync(root, branches).await
}

pub struct Service<'a> {
    preparation: &'a Preparation,
    client: Client<'a>,
}

impl<'a> Service<'a> {
    pub(super) async fn open(
        preparation: &'a Preparation,
        identity: &Path,
        client: Client<'a>,
    ) -> Result<Self> {
        let observed = super::adaptor::repository(identity).await?;
        if observed != client.repository {
            return Err(stale(format!(
                "delivery repository {} is not worktree origin {observed}",
                client.repository
            )));
        }
        Ok(Self {
            preparation,
            client,
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
            let candidate = &self.preparation.candidate;
            pulls.retain(|pull| {
                pull.state == State::Open || (merged(pull) && &pull.head == candidate)
            });
            if pulls.len() > 1 {
                return Err(stale("more than one pull matches the exact projection"));
            }
            pulls.pop()
        };
        match pull {
            Some(pull) if pull.state == State::Open => Ok(Some(pull)),
            pull => pull.map(|pull| self.validate(pull)).transpose(),
        }
    }

    pub fn current(&self, pull: &Pull) -> bool {
        super::adaptor::contract(pull, self.preparation, self.client.repository).is_ok()
    }

    pub async fn refresh(&mut self, number: i64) -> Result<Pull> {
        self.client
            .edit(number, &self.preparation.title, &self.preparation.body)
            .await?;
        self.view(number).await
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
        match Git::run(root, &arguments).await {
            Ok(_) => Ok(()),
            Err(error) if rejected(error.message()) => Err(stale(error.message())),
            Err(error) => Err(error),
        }
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

    pub async fn mark(&mut self, authority: Mode) -> Result<()> {
        self.client
            .mark(&self.preparation.candidate, authority)
            .await
    }

    pub async fn settle(&mut self, number: i64, squash: &plumb::delivery::Squash) -> Result<Merge> {
        self.client.settle(number, squash).await
    }

    pub async fn pending(&mut self, number: i64) -> Result<bool> {
        let checks = self.client.checks(number).await?;
        super::settle::failures(&checks)?;
        Ok(!checks.pending.is_empty())
    }
    pub fn report(&self, pull: Pull) -> Report {
        Report {
            pull,
            candidate: self.preparation.candidate.clone(),
        }
    }

    fn validate(&self, pull: Pull) -> Result<Pull> {
        super::adaptor::contract(&pull, self.preparation, self.client.repository)?;
        Ok(pull)
    }

    fn reference(&self, reference: &Reference) -> Result<()> {
        super::adaptor::validate(reference, self.client.repository)
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

pub(crate) fn stale(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.stale", message)
}

pub(crate) fn provider(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.provider", message)
}

fn merged(pull: &Pull) -> bool {
    pull.merge.is_some() || pull.merged.is_some()
}

fn rejected(message: &str) -> bool {
    message.contains("(stale info)") || message.contains("(fetch first)")
}

pub(super) use super::adaptor::reference;

#[cfg(test)]
mod tests {
    use super::{Service, sync};
    use crate::dispatch::forge::delivery::adaptor::Client;
    use concord_core::delivery::Preparation;
    use std::path::Path;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn commit(root: &Path, file: &str, body: &str) -> String {
        std::fs::write(root.join(file), body).expect("write");
        git(root, &["add", file]);
        git(
            root,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "c",
            ],
        );
        git(root, &["rev-parse", "HEAD"])
    }

    fn prep(root: &Path, base: &str, head: &str) -> Preparation {
        Preparation {
            root: root.to_path_buf(),
            base: "main".into(),
            target: base.into(),
            branch: "task/1".into(),
            source: head.into(),
            projection: "land/task/1".into(),
            candidate: head.into(),
            title: "t".into(),
            body: "b".into(),
        }
    }

    async fn push(preparation: &Preparation) -> concord_core::Result<()> {
        let client = Client {
            repository: "PerishLab/probe",
            timeout: 10,
        };
        Service {
            preparation,
            client,
        }
        .publish()
        .await
    }

    #[tokio::test]
    async fn lease() {
        let dir = tempfile::tempdir().expect("dir");
        let origin = dir.path().join("origin.git");
        let work = dir.path().join("work");
        let peer = dir.path().join("peer");
        git(dir.path(), &["init", "--bare", origin.to_str().unwrap()]);
        git(dir.path(), &["init", work.to_str().unwrap()]);
        git(&work, &["checkout", "-B", "main"]);
        let base = commit(&work, "README.md", "base\n");
        git(
            &work,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&work, &["push", "-u", "origin", "main"]);
        let narrow = "+refs/heads/main:refs/remotes/origin/main";
        git(&work, &["config", "remote.origin.fetch", narrow]);
        git(&work, &["checkout", "-b", "task/1"]);
        let first = commit(&work, "topic.md", "one\n");
        let pair = [
            format!("{first}:refs/heads/task/1"),
            format!("{first}:refs/heads/land/task/1"),
        ];
        git(&work, &["push", "origin", &pair[0], &pair[1]]);
        let rewritten = commit(&work, "topic.md", "two\n");
        sync(&work, &["task/1", "land/task/1"]).await.expect("sync");
        push(&prep(&work, &base, &rewritten))
            .await
            .expect("publish");
        assert_eq!(git(&origin, &["rev-parse", "refs/heads/task/1"]), rewritten);

        git(
            dir.path(),
            &["clone", origin.to_str().unwrap(), peer.to_str().unwrap()],
        );
        git(&peer, &["checkout", "-b", "task/1", "origin/task/1"]);
        let foreign = commit(&peer, "topic.md", "foreign\n");
        git(&peer, &["push", "origin", "task/1"]);
        let third = commit(&work, "topic.md", "three\n");
        let error = push(&prep(&work, &base, &third)).await.expect_err("stale");
        assert_eq!(error.code(), "concord.delivery.stale");
        assert_eq!(git(&origin, &["rev-parse", "refs/heads/task/1"]), foreign);
    }
}
