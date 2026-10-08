use super::{emit, explicit};
use crate::args::member::Command;
use concord_core::{
    Coordinate, Estate, Finish, IssueClaiming, IssueMemberChange, IssueNarrowing, IssueProving,
    IssueRelease, IssueRetirement, Result, Start,
};
use serde_json::json;

mod landing;
mod reference;
mod start;
mod status;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { issue } => list(estate, issue, output).await,
        Command::Status { issue } => status::run(estate, &issue, output).await,
        Command::Start {
            issue,
            claim,
            revision,
            command,
            timeout,
        } => {
            let issue = Coordinate::parse(&issue)?;
            let observed = start::observe(&issue, &command, timeout).await?;
            issue_changed(
                estate
                    .start(&Start {
                        issue,
                        node: observed.node,
                        stable: observed.stable,
                        kind: observed.kind,
                        claims: claim,
                        revision,
                    })
                    .await?,
                output,
            )
        }
        Command::Claim {
            issue,
            claim,
            revision,
        } => issue_changed(
            estate
                .claim_issue(&IssueClaiming {
                    issue: Coordinate::parse(&issue)?,
                    claims: claim,
                    revision,
                })
                .await?,
            output,
        ),
        Command::Narrow {
            issue,
            claim,
            revision,
            apply,
        } => {
            explicit(apply, "member narrow")?;
            issue_changed(
                estate
                    .narrow_issue(&IssueNarrowing {
                        issue: Coordinate::parse(&issue)?,
                        claims: claim,
                        revision,
                    })
                    .await?,
                output,
            )
        }
        Command::Prove { issue, revision } => emit(
            json!({"member":
            estate.prove_issue(&IssueProving {
                issue: Coordinate::parse(&issue)?, revision,
            }).await?}),
            output,
        ),
        Command::Landing { command } => landing::run(estate, command, output).await,
        Command::Reference { command } => reference::run(estate, command, output).await,
        Command::Release {
            issue,
            revision,
            apply,
        } => {
            explicit(apply, "member release")?;
            let revision = estate
                .release_issue(&IssueRelease {
                    issue: Coordinate::parse(&issue)?,
                    revision,
                })
                .await?;
            emit(json!({"revision": revision}), output)
        }
        Command::Complete {
            issue,
            revision,
            apply,
        } => {
            explicit(apply, "member complete")?;
            let revision = estate
                .finish(&Finish {
                    issue: Coordinate::parse(&issue)?,
                    revision,
                })
                .await?;
            emit(json!({"revision": revision}), output)
        }
        Command::Retire {
            issue,
            artifacts,
            revision,
            apply,
        } => {
            explicit(apply, "member retire")?;
            let revision = estate
                .retire_issue(&IssueRetirement {
                    issue: Coordinate::parse(&issue)?,
                    artifacts,
                    revision,
                })
                .await?;
            emit(json!({"revision": revision}), output)
        }
    }
}

async fn list(estate: &Estate, issue: Option<String>, output: bool) -> Result<()> {
    let mut members = estate.issue_worktrees().await?;
    if let Some(issue) = issue {
        let node = estate.issue(&Coordinate::parse(&issue)?).await?.node;
        members.retain(|member| member.node == node);
    }
    emit(json!({"members": members}), output)
}

fn issue_changed(change: IssueMemberChange, output: bool) -> Result<()> {
    if !output {
        super::output::observations(&change.observations);
    }
    emit(
        json!({"member": change.member, "observations": change.observations}),
        output,
    )
}
