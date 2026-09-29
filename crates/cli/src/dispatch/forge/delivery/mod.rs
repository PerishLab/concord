mod git;
mod projection;
mod pull;
mod settle;
#[cfg(test)]
mod squash;

use super::super::{emit, input};
use super::projection::Projection;
use crate::args::issue::Delivery;
use concord_core::authority::Plumb;
use concord_core::{Coordinate, Error, Estate, IssueDeclaration, Result, issue_delivery};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u64,
    plan: issue_delivery::Plan,
}

pub async fn run(estate: &Estate, command: Delivery, output: bool) -> Result<()> {
    match command {
        Delivery::Prepare {
            issue,
            revision,
            base,
            command,
            timeout,
        } => {
            let issue = Coordinate::parse(&issue)?;
            let observed = Projection::new(&command, 100, 1, timeout)
                .delivery(&issue)
                .await?;
            let plan = issue_delivery::prepare::<Plumb>(
                estate,
                &issue_delivery::Request {
                    issue,
                    revision,
                    base,
                    snapshot: observed.snapshot,
                    observed: observed.observed,
                    outcome: observed.outcome,
                },
            )
            .await?;
            emit(json!({"plan": plan}), output)
        }
        command @ Delivery::Land { .. } => land(estate, command, output).await,
    }
}

async fn land(estate: &Estate, command: Delivery, output: bool) -> Result<()> {
    let Delivery::Land {
        issue,
        plan: path,
        command,
        timeout,
    } = command
    else {
        return Err(Error::typed(
            "concord.delivery.command",
            "delivery land received a non-land command",
        ));
    };
    let envelope: Envelope = input::read(&path, SHAPE)?;
    version(envelope.version)?;
    let plan = envelope.plan;
    if plan.issue != Coordinate::parse(&issue)? {
        return Err(Error::typed(
            "concord.delivery.coordinate",
            "delivery plan Issue or Member does not match the command",
        ));
    }
    pull::fetch(&plan.delivery.root).await?;
    let observed = Projection::new(&command, 100, 1, timeout)
        .delivery(&plan.issue)
        .await?;
    let ready =
        issue_delivery::revalidate::<Plumb>(estate, &plan, &observed.snapshot, observed.observed)
            .await?;
    let mut report = pull::land(
        &ready.preparation,
        &plan.delivery.repository,
        &command,
        timeout,
    )
    .await?;
    let revision = attach(estate, &plan, &report.pull).await?;
    settle::merge(&mut report, &ready.preparation, &command, timeout).await?;
    emit(
        json!({
            "schema": "concord.issue-delivery-land/v2",
            "issue": plan.issue,
            "revision": revision,
            "pull": report.pull,
            "candidate": report.candidate,
            "merged": report.merged,
        }),
        output,
    )
}

async fn attach(estate: &Estate, plan: &issue_delivery::Plan, pull: &pull::Pull) -> Result<i64> {
    let status = estate.issue_member_status(&plan.issue).await?;
    let coordinate = (plan.delivery.repository.split_once('/'), pull.number);
    let Some((owner, repository)) = coordinate.0 else {
        return Err(Error::typed(
            "concord.delivery.repository",
            "delivery repository coordinate is malformed",
        ));
    };
    if status.references.iter().any(|reference| {
        reference.owner == owner
            && reference.repository == repository
            && reference.number == pull.number
    }) {
        return Ok(estate.issue(&plan.issue).await?.revision);
    }
    if !status.references.is_empty() {
        return Err(Error::typed(
            "concord.delivery.reference",
            "Member already carries a different pull coordinate",
        ));
    }
    estate
        .refer_issue(&IssueDeclaration {
            issue: plan.issue.clone(),
            provider: "github".to_string(),
            owner: owner.to_string(),
            repository: repository.to_string(),
            number: pull.number,
            revision: plan.revision,
        })
        .await
}

fn version(value: u64) -> Result<()> {
    if value == 1 {
        return Ok(());
    }
    Err(Error::typed(
        "concord.delivery.envelope",
        "delivery envelope version must be 1",
    ))
}

const SHAPE: &str = r#"{"version":1,"plan":{"schema":"concord.issue-member-delivery/v3","issue":{"owner":"OWNER","repository":"REPOSITORY","number":1},"node":"I_node","revision":1,"member":{},"boundary":{},"authority":{},"delivery":{}}}"#;

#[cfg(test)]
mod tests {
    #[test]
    fn version() {
        super::version(1).expect("version one");
        assert_eq!(
            super::version(2).expect_err("other version").code(),
            "concord.delivery.envelope"
        );
    }
}
