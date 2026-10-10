use super::client::Client;
use super::git::Git;
use super::model::Checks;
use super::pull::{Pull, State, stale};
use concord_core::issue_delivery::Preparation;
use concord_core::{Error, Result};
use plumb::delivery::Squash;
use std::time::Duration;

pub const BUDGET: Duration = Duration::from_secs(30 * 60);
const FIRST: Duration = Duration::from_secs(2);
const CEILING: Duration = Duration::from_secs(30);
const CYCLES: u32 = 20;

pub async fn exact(preparation: &Preparation) -> Result<()> {
    Git::fetch(&preparation.root).await?;
    let source = Git::run(&preparation.root, &["rev-parse", "HEAD"]).await?;
    let target = Git::run(
        &preparation.root,
        &["rev-parse", &format!("origin/{}", preparation.base)],
    )
    .await?;
    if source != preparation.source || target != preparation.target {
        return Err(stale(
            "source or base advanced before the next provider mutation",
        ));
    }
    Ok(())
}

pub fn squash(preparation: &Preparation) -> Result<Squash> {
    Squash::read(&preparation.root, &preparation.candidate).map_err(|refusal| {
        Error::typed(
            "concord.delivery.squash",
            format!(
                "cannot derive the squash for candidate {}: {refusal}",
                preparation.candidate
            ),
        )
    })
}

pub fn merged(pull: &Pull) -> Result<String> {
    match pull.state {
        State::Merged => pull
            .merge
            .as_ref()
            .map(|commit| commit.oid.clone())
            .filter(|commit| !commit.is_empty())
            .ok_or_else(|| stale("merged pull has no merge commit")),
        State::Closed => Err(stale("exact pull closed without merging")),
        State::Open => Err(Error::typed(
            "concord.delivery.incomplete",
            "merge command returned without merged-state provider readback",
        )),
    }
}

pub enum Step {
    Retry,
    Wait(Duration),
}

pub struct Wait {
    elapsed: Duration,
    delay: Duration,
    cycles: u32,
}

impl Wait {
    pub fn new() -> Self {
        Self {
            elapsed: Duration::ZERO,
            delay: FIRST,
            cycles: 0,
        }
    }

    pub fn step(&mut self, checks: &Checks) -> Result<Step> {
        if checks.pending.is_empty() {
            self.cycles += 1;
            if self.cycles > CYCLES {
                return Err(exhausted(format!(
                    "merge stayed pending through {CYCLES} settled check cycles"
                )));
            }
            self.delay = FIRST;
            return Ok(Step::Retry);
        }
        if self.elapsed >= BUDGET {
            return Err(exhausted(format!(
                "pull checks still pending after {} minutes: {}",
                BUDGET.as_secs() / 60,
                checks.pending.join(", ")
            )));
        }
        let delay = self.delay;
        self.elapsed += delay;
        self.delay = (delay * 2).min(CEILING);
        Ok(Step::Wait(delay))
    }
}

pub async fn wait(client: &mut Client<'_>, pull: i64, wait: &mut Wait) -> Result<()> {
    loop {
        match wait.step(&client.checks(pull).await?)? {
            Step::Retry => return Ok(()),
            Step::Wait(delay) => tokio::time::sleep(delay).await,
        }
    }
}

fn exhausted(message: String) -> Error {
    Error::typed("concord.delivery.pending", message)
}

#[cfg(test)]
mod tests {
    use super::super::model::Checks;
    use super::super::pull::{Commit, Pull, State};
    use super::{BUDGET, CEILING, FIRST, Step, Wait, merged};

    #[test]
    fn readback() {
        let exact = pull(State::Merged, Some("merge"));
        assert_eq!(merged(&exact).expect("merged evidence"), "merge");

        let incomplete = pull(State::Open, None);
        assert_eq!(
            merged(&incomplete).expect_err("open is incomplete").code(),
            "concord.delivery.incomplete"
        );

        let closed = pull(State::Closed, None);
        assert_eq!(
            merged(&closed).expect_err("closed is stale").code(),
            "concord.delivery.stale"
        );
    }

    #[test]
    fn evidence() {
        let pull = pull(State::Merged, None);
        assert_eq!(
            merged(&pull).expect_err("missing merge commit").code(),
            "concord.delivery.stale"
        );
    }

    fn pull(state: State, merge: Option<&str>) -> Pull {
        Pull {
            id: "PR_node".into(),
            number: 7,
            url: "https://github.com/PerishLab/probe/pull/7".into(),
            state,
            base: "main".into(),
            head: "candidate".into(),
            merge: merge.map(|oid| Commit { oid: oid.into() }),
            merged: merge.map(|_| "2026-09-29T00:00:00Z".into()),
            updated: "2026-09-29T00:00:01Z".into(),
            title: "Deliver topic".into(),
            body: "Refs PerishLab/probe#1".into(),
        }
    }

    #[test]
    fn budget() {
        let pending = Checks {
            pending: vec!["Guard".into()],
            failed: Vec::new(),
        };
        let mut wait = Wait::new();
        let mut waited = std::time::Duration::ZERO;
        let mut delays = Vec::new();
        let refusal = loop {
            match wait.step(&pending) {
                Ok(Step::Wait(delay)) => {
                    waited += delay;
                    delays.push(delay);
                }
                Ok(Step::Retry) => panic!("pending checks must not retry"),
                Err(refusal) => break refusal,
            }
        };
        assert_eq!(delays[0], FIRST);
        assert!(delays.iter().all(|delay| *delay <= CEILING));
        assert!(waited >= BUDGET && waited < BUDGET + CEILING);
        assert_eq!(refusal.code(), "concord.delivery.pending");
        assert!(refusal.message().contains("Guard"));
        assert!(matches!(wait.step(&Checks::default()), Ok(Step::Retry)));
    }

    #[test]
    fn cycles() {
        let mut wait = Wait::new();
        for _ in 0..super::CYCLES {
            assert!(matches!(wait.step(&Checks::default()), Ok(Step::Retry)));
        }
        let refusal = wait
            .step(&Checks::default())
            .err()
            .expect("settled cycles are bounded");
        assert_eq!(refusal.code(), "concord.delivery.pending");
    }
}
