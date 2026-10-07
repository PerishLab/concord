use serde::{Deserialize, Serialize};

use super::{Comment, History, Judgment, Marker, Reference, Target, Verification};
use crate::Result;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Satisfied,
    Unmet,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub verdict: Verdict,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub name: String,
    pub fact: Fact,
}

pub struct Observation<'a> {
    pub labels: &'a [String],
    pub comments: &'a [Comment],
    pub body: &'a str,
    pub freshness: Fact,
    pub conditions: Vec<Check>,
    pub release: Fact,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Evaluation {
    pub target: Option<Target>,
    pub declaration: Option<Reference>,
    pub closure: Option<Reference>,
    pub verification: Option<Verification>,
    pub verdict: Verdict,
    pub checks: Vec<Check>,
}

impl Evaluation {
    pub fn read(observation: Observation<'_>) -> Result<Self> {
        let history = History::read(observation.comments)?;
        let target = history.target(observation.labels)?;
        let mut evaluation = Self {
            target,
            declaration: history.declaration.map(|record| record.reference),
            closure: history
                .closure
                .as_ref()
                .map(|record| record.reference.clone()),
            verification: None,
            verdict: Verdict::Unknown,
            checks: Vec::new(),
        };
        evaluation.assess(history.closure.map(|record| record.marker), observation)?;
        evaluation.verdict = evaluation.aggregate();
        Ok(evaluation)
    }

    fn review(&mut self, marker: Option<Marker>, body: &str) {
        let Some(Marker::Closure {
            judgment,
            verification,
            review,
            remaining,
            ..
        }) = marker
        else {
            self.push("closure", Verdict::Unknown, "no current closure judgment");
            return;
        };
        self.verification = Some(verification);
        self.push(
            "judgment",
            if judgment == Judgment::Satisfied {
                Verdict::Satisfied
            } else {
                Verdict::Unmet
            },
            if remaining.is_empty() {
                "closure declares satisfaction".into()
            } else {
                remaining.join("; ")
            },
        );
        self.push(
            "review",
            if review == super::digest(body) {
                Verdict::Satisfied
            } else {
                Verdict::Unmet
            },
            "closure review must bind the exact current Issue body",
        );
    }

    fn assess(&mut self, marker: Option<Marker>, observation: Observation<'_>) -> Result<()> {
        self.review(marker, observation.body);
        self.fact("freshness", observation.freshness)?;
        if observation.conditions.is_empty() {
            self.push(
                "conditions",
                Verdict::Unknown,
                "detailed acceptance facts were not observed",
            );
        }
        for check in observation.conditions {
            self.fact(&check.name, check.fact)?;
        }
        if self.target == Some(Target::Release) {
            self.fact("release", observation.release)?;
        }
        Ok(())
    }

    fn fact(&mut self, name: &str, fact: Fact) -> Result<()> {
        if name.trim().is_empty() || fact.reason.trim().is_empty() {
            return Err(super::fault(
                "observed acceptance facts require names and reasons",
            ));
        }
        if self.checks.iter().any(|check| check.name == name) {
            return Err(super::fault("repeated acceptance check name"));
        }
        self.push(name, fact.verdict, fact.reason);
        Ok(())
    }

    fn push(&mut self, name: &str, verdict: Verdict, reason: impl Into<String>) {
        self.checks.push(Check {
            name: name.into(),
            fact: Fact {
                verdict,
                reason: reason.into(),
            },
        });
    }

    fn aggregate(&self) -> Verdict {
        if self
            .checks
            .iter()
            .any(|check| check.fact.verdict == Verdict::Unmet)
        {
            return Verdict::Unmet;
        }
        if self
            .checks
            .iter()
            .any(|check| check.fact.verdict == Verdict::Unknown)
        {
            return Verdict::Unknown;
        }
        Verdict::Satisfied
    }
}
