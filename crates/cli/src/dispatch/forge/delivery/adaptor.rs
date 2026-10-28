use super::super::github;
use concord_core::{Coordinate, Error, Integration, Result, issue_delivery};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(super) use github::delivery::{Client, Merge};
pub(super) use github::identity::reference;
pub(super) use github::identity::validate;
pub(super) use github::observation::Observation;
#[cfg(test)]
pub(super) use github::pull::Commit;
pub(super) use github::pull::{Checks, Pull, Report, State, contract};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    kind: String,
    host: String,
    repository: String,
    node: String,
}

pub(super) fn select(kind: &str) -> Result<()> {
    if kind != "github" {
        return Err(unsupported(format!(
            "forge {kind:?} has no qualified delivery adaptor; use --forge github for github.com"
        )));
    }
    Ok(())
}

impl Identity {
    pub(super) fn of(plan: &issue_delivery::Plan) -> Self {
        Self {
            kind: "github".into(),
            host: "github.com".into(),
            repository: plan.delivery.repository.clone(),
            node: plan.member.integration.node.clone(),
        }
    }

    pub(super) fn validate(
        version: u64,
        identity: Option<&Self>,
        plan: &issue_delivery::Plan,
    ) -> Result<()> {
        match (version, identity) {
            (1, None) => Ok(()),
            (2, Some(identity)) if identity == &Self::of(plan) => Ok(()),
            _ => Err(unsupported(
                "delivery adaptor identity is missing, unsupported or disagrees with the plan; prepare a fresh plan with --forge github",
            )),
        }
    }
}

pub(super) async fn integration(integration: &Integration) -> Result<()> {
    select("github")?;
    let observed = repository(Path::new(&integration.path)).await?;
    let expected = format!(
        "{}/{}",
        integration.repository.owner, integration.repository.name
    );
    if observed != expected {
        return Err(unsupported(
            "GitHub origin disagrees with the registered Integration",
        ));
    }
    Ok(())
}

pub(super) async fn repository(root: &Path) -> Result<String> {
    github::identity::repository(root).await
}

pub(super) async fn observe(
    command: &Path,
    timeout: u64,
    issue: &Coordinate,
) -> Result<Observation> {
    select("github")?;
    super::super::projection::Projection::new(command, 100, 1, timeout)
        .delivery(issue)
        .await
}

fn unsupported(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.adaptor", message)
}
