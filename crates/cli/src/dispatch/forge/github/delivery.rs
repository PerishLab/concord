use super::super::delivery::pull::provider;
use super::identity::{Native, diagnostic};
use super::pull::{Checks, Pull};
use crate::runtime::github;
use concord_core::Result;
use std::time::Duration;

pub(crate) struct Client<'a> {
    pub repository: &'a str,
    pub timeout: u64,
}

impl Client<'_> {
    pub async fn all(&mut self, base: &str, head: &str) -> Result<Vec<Pull>> {
        Native::open(self.repository, self.timeout)?.listing(base, head)
    }

    pub async fn raise(&mut self, base: &str, head: &str, title: &str, body: &str) -> Result<Pull> {
        Native::open(self.repository, self.timeout)?.raise(base, head, title, body)
    }

    pub async fn edit(&mut self, pull: i64, title: &str, body: &str) -> Result<()> {
        Native::open(self.repository, self.timeout)?.edit(pull, title, body)
    }

    pub async fn view(&mut self, pull: i64) -> Result<Pull> {
        Native::open(self.repository, self.timeout)?.view(pull)
    }

    pub async fn mark(
        &mut self,
        candidate: &str,
        authority: concord_core::delivery::Mode,
    ) -> Result<()> {
        let (context, description) = match authority {
            concord_core::delivery::Mode::Plumb => (
                "guard / guard (pull_request)",
                "Concord revalidated the exact Plumb Guard proof",
            ),
            concord_core::delivery::Mode::Manual => (
                "manual / governance (pull_request)",
                "Concord revalidated exact manual review inputs, not automated tests",
            ),
            concord_core::delivery::Mode::Wharf => (
                "native / wharf (pull_request)",
                "Concord revalidated the exact Wharf native proof",
            ),
        };
        Native::open(self.repository, self.timeout)?.mark(candidate, context, description)
    }

    pub async fn settle(&mut self, pull: i64, squash: &plumb::delivery::Squash) -> Result<Merge> {
        let native = Native::open(self.repository, self.timeout)?;
        self.squash(&native, pull, squash).await
    }

    async fn squash(
        &mut self,
        native: &Native<'_>,
        pull: i64,
        squash: &plumb::delivery::Squash,
    ) -> Result<Merge> {
        use runseal::github::{Outcome, Squash};
        let target = native.target(pull)?;
        let input = Squash::new(&squash.candidate, Some(&squash.subject), Some(&squash.body))
            .map_err(diagnostic)?;
        let mut last = None;
        for turn in 1..=6 {
            let error = match github::call("pull.squash", || native.client.squash(&target, &input))
            {
                Ok(_) => return Ok(Merge::Merged),
                Err(error)
                    if error.outcome == Outcome::Refused && error.evidence.status == Some(405) =>
                {
                    diagnostic(error)
                }
                Err(error) => return Err(diagnostic(error)),
            };
            let checks = self.checks(pull).await?;
            if !checks.pending.is_empty() {
                return Ok(Merge::Pending);
            }
            if !checks.failed.is_empty() {
                return Err(failed(&error, &checks.failed));
            }
            last = Some(error);
            crate::observation::wait("mergeability", Duration::from_secs(turn)).await;
        }
        Err(last.unwrap_or_else(|| provider("pull did not become mergeable")))
    }

    pub async fn checks(&mut self, pull: i64) -> Result<Checks> {
        Native::open(self.repository, self.timeout)?.checks(pull)
    }
}

fn failed(error: &concord_core::Error, checks: &[String]) -> concord_core::Error {
    provider(format!(
        "{} (failed checks: {})",
        error.message(),
        checks.join(", ")
    ))
}

pub enum Merge {
    Merged,
    Pending,
}
