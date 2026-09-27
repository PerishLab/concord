mod git;
mod projection;
mod pull;

use super::super::{emit, input};
use super::projection::Projection;
use crate::args::issue::Delivery;
use concord_core::{Coordinate, Error, Estate, IssueDeclaration, Result, issue_delivery};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    plan: issue_delivery::Plan,
}

pub async fn run(estate: &Estate, command: Delivery, output: bool) -> Result<()> {
    match command {
        Delivery::Prepare {
            issue,
            member,
            revision,
            base,
            command,
            timeout,
        } => {
            let issue = Coordinate::parse(&issue)?;
            let observed = Projection::new(&command, 100, 1, timeout)
                .delivery(&issue)
                .await?;
            let plan = issue_delivery::prepare(
                estate,
                &issue_delivery::Request {
                    issue,
                    member,
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
        member,
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
    let plan = envelope.plan;
    if plan.issue != Coordinate::parse(&issue)? || plan.member.name != member {
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
        issue_delivery::revalidate(estate, &plan, &observed.snapshot, observed.observed).await?;
    let mut report = pull::land(
        &ready.preparation,
        &plan.delivery.repository,
        &command,
        timeout,
    )
    .await?;
    let revision = attach(estate, &plan, &report.pull).await?;
    pull::merge(&mut report, &ready.preparation, &command, timeout).await?;
    emit(
        json!({
            "schema": "concord.issue-delivery-land/v1",
            "issue": plan.issue,
            "member": plan.member.name,
            "revision": revision,
            "pull": report.pull,
            "candidate": report.candidate,
            "merged": report.merged,
        }),
        output,
    )
}

async fn attach(estate: &Estate, plan: &issue_delivery::Plan, pull: &pull::Pull) -> Result<i64> {
    let status = estate
        .issue_member_status(&plan.issue, &plan.member.name)
        .await?;
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
            member: plan.member.name.clone(),
            provider: "github".to_string(),
            owner: owner.to_string(),
            repository: repository.to_string(),
            number: pull.number,
            revision: plan.revision,
        })
        .await
}

const SHAPE: &str = r#"{"plan":{"schema":"concord.issue-member-delivery/v1","issue":{"owner":"OWNER","repository":"REPOSITORY","number":1},"node":"I_node","revision":1,"member":{},"boundary":{},"delivery":{}}}"#;
