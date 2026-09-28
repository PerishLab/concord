use super::{emit, explicit};
use crate::args::member::Command;
use concord_core::{
    Coordinate, Estate, IssueAttach, IssueClaiming, IssueMemberChange, IssueNarrowing,
    IssueProving, IssueRelease, IssueRetirement, Result,
};
use serde_json::json;

mod landing;
mod reference;
mod status;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { issue } => list(estate, issue, output).await,
        Command::Status { issue, member } => status::run(estate, &issue, &member, output).await,
        Command::Attach {
            issue,
            name,
            source,
            branch,
            claim,
            revision,
        } => issue_changed(
            estate
                .attach_issue(&IssueAttach {
                    issue: Coordinate::parse(&issue)?,
                    name,
                    source,
                    branch,
                    claims: claim,
                    revision,
                })
                .await?,
            output,
        ),
        Command::Claim {
            issue,
            member,
            claim,
            revision,
        } => issue_changed(
            estate
                .claim_issue(&IssueClaiming {
                    issue: Coordinate::parse(&issue)?,
                    member,
                    claims: claim,
                    revision,
                })
                .await?,
            output,
        ),
        Command::Narrow {
            issue,
            member,
            claim,
            revision,
            apply,
        } => {
            explicit(apply, "member narrow")?;
            issue_changed(
                estate
                    .narrow_issue(&IssueNarrowing {
                        issue: Coordinate::parse(&issue)?,
                        member,
                        claims: claim,
                        revision,
                    })
                    .await?,
                output,
            )
        }
        Command::Prove {
            issue,
            member,
            revision,
        } => emit(
            json!({"member":
            estate.prove_issue(&IssueProving {
                issue: Coordinate::parse(&issue)?, member, revision,
            }).await?}),
            output,
        ),
        Command::Landing { command } => landing::run(estate, command, output).await,
        Command::Reference { command } => reference::run(estate, command, output).await,
        Command::Release {
            issue,
            member,
            revision,
            apply,
        } => {
            explicit(apply, "member release")?;
            let revision = estate
                .release_issue(&IssueRelease {
                    issue: Coordinate::parse(&issue)?,
                    member,
                    revision,
                })
                .await?;
            emit(json!({"revision": revision}), output)
        }
        Command::Retire {
            issue,
            member,
            artifacts,
            revision,
            apply,
        } => {
            explicit(apply, "member retire")?;
            let revision = estate
                .retire_issue(&IssueRetirement {
                    issue: Coordinate::parse(&issue)?,
                    member,
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
