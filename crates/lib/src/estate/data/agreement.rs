use super::{Anchor, Coordinate, Estate, Integration, IssueWorktree};
use crate::{Error, Result};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub code: String,
    pub subject: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Agreement {
    pub target: String,
    pub revision: i64,
    pub faults: Vec<Finding>,
    pub observations: Vec<Finding>,
}

impl Agreement {
    pub fn agrees(&self) -> bool {
        self.faults.is_empty()
    }

    pub(super) fn fault(
        &mut self,
        code: &str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.faults.push(Finding {
            code: code.to_string(),
            subject: subject.into(),
            message: message.into(),
        });
    }

    pub(super) fn observe(
        &mut self,
        code: &str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.observations.push(Finding {
            code: code.to_string(),
            subject: subject.into(),
            message: message.into(),
        });
    }
}

#[derive(Clone, Copy)]
pub(in crate::estate) enum Scope<'a> {
    All,
    Control,
    Seat(&'a Coordinate),
    Issue(&'a Coordinate),
    Repository(&'a Integration),
}

impl Scope<'_> {
    pub(super) fn anchor(self, anchor: &Anchor) -> bool {
        match self {
            Self::All => true,
            Self::Control => false,
            Self::Seat(issue) | Self::Issue(issue) => issue == &anchor.coordinate,
            Self::Repository(held) => {
                held.repository.owner == anchor.coordinate.owner
                    && held.repository.name == anchor.coordinate.repository
            }
        }
    }

    pub(super) fn member(self, member: &IssueWorktree) -> bool {
        match self {
            Self::All => true,
            Self::Control | Self::Seat(_) => false,
            Self::Issue(issue) => issue == &member.issue,
            Self::Repository(held) => held.key == member.integration.key,
        }
    }

    pub(super) fn integration(self, integration: &Integration) -> bool {
        match self {
            Self::All => true,
            Self::Control | Self::Seat(_) => false,
            Self::Issue(issue) => {
                issue.owner == integration.repository.owner
                    && issue.repository == integration.repository.name
            }
            Self::Repository(held) => held.key == integration.key,
        }
    }

    fn description(self) -> serde_json::Value {
        match self {
            Self::All => serde_json::json!({"kind":"estate"}),
            Self::Control => serde_json::json!({"kind":"control"}),
            Self::Seat(issue) => serde_json::json!({"kind":"seat", "issue":issue}),
            Self::Issue(issue) => serde_json::json!({"kind":"issue", "issue":issue}),
            Self::Repository(held) => {
                serde_json::json!({"kind":"repository", "node":held.node, "key":held.key})
            }
        }
    }

    pub(super) fn complete(self) -> bool {
        matches!(self, Self::All)
    }
}

impl Estate {
    #[locus::trace(with = plumb::observation::view())]
    pub async fn inspect(&self) -> Result<Agreement> {
        self.survey(Scope::All).await
    }

    pub(in crate::estate) async fn survey(&self, scope: Scope<'_>) -> Result<Agreement> {
        let mut report = Agreement {
            target: self.layout.root.display().to_string(),
            revision: 0,
            faults: Vec::new(),
            observations: Vec::new(),
        };
        super::territory::inspect(self, &mut report, scope).await?;
        report.faults.sort_by_key(order);
        report.observations.sort_by_key(order);
        Ok(report)
    }

    pub async fn settled(&self) -> Result<Agreement> {
        let _shared = self.share()?;
        self.inspect().await
    }

    pub(super) async fn ensure(&self, scope: Scope<'_>) -> Result<()> {
        verdict(self.survey(scope).await?, scope)
    }

    pub(super) async fn agreed(&self, scope: Scope<'_>) -> Result<()> {
        let _shared = self.share()?;
        verdict(self.survey(scope).await?, scope)
    }
}

fn verdict(report: Agreement, scope: Scope<'_>) -> Result<()> {
    if report.agrees() {
        return Ok(());
    }
    let count = report.faults.len();
    Err(Error::detailed(
        "concord.audit.refused",
        format!("required scope has {count} agreement fault(s)"),
        serde_json::json!({"agreement": report, "scope": scope.description()}),
    ))
}

fn order(finding: &Finding) -> (String, String, String) {
    (
        finding.code.clone(),
        finding.subject.clone(),
        finding.message.clone(),
    )
}
