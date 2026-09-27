use super::{emit, explicit};
use crate::args::member::Command;
use concord_core::{
    Attach, Claiming, Coordinate, Estate, IssueAttach, IssueClaiming, IssueMemberChange,
    IssueNarrowing, IssueProving, IssueRelease, IssueRetirement, MemberChange, Narrowing, Proving,
    Release, Result, Retirement,
};
use serde_json::json;

mod landing;
mod reference;
mod status;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { task, issue } => list(estate, task, issue, output).await,
        Command::Status {
            task,
            member,
            observation,
        } => status::run(estate, (&task, &member), observation, output).await,
        Command::Attach {
            task,
            name,
            source,
            branch,
            claim,
            revision,
        } => {
            if issue(&task) {
                let request = IssueAttach {
                    issue: Coordinate::parse(&task)?,
                    name,
                    source,
                    branch,
                    claims: claim,
                    revision,
                };
                issue_changed(estate.attach_issue(&request).await?, output)
            } else {
                let request = Attach {
                    task,
                    name,
                    source,
                    branch,
                    claims: claim,
                    revision,
                };
                changed(estate.attach(&request).await?, output)
            }
        }
        Command::Claim {
            task,
            member,
            claim,
            revision,
        } => {
            if issue(&task) {
                let request = IssueClaiming {
                    issue: Coordinate::parse(&task)?,
                    member,
                    claims: claim,
                    revision,
                };
                issue_changed(estate.claim_issue(&request).await?, output)
            } else {
                let request = Claiming {
                    task,
                    member,
                    claims: claim,
                    revision,
                };
                changed(estate.claim(&request).await?, output)
            }
        }
        Command::Narrow {
            task,
            member,
            claim,
            revision,
            apply,
        } => {
            explicit(apply, "member narrow")?;
            if issue(&task) {
                let request = IssueNarrowing {
                    issue: Coordinate::parse(&task)?,
                    member,
                    claims: claim,
                    revision,
                };
                issue_changed(estate.narrow_issue(&request).await?, output)
            } else {
                let request = Narrowing {
                    task,
                    member,
                    claims: claim,
                    revision,
                };
                changed(estate.narrow(&request).await?, output)
            }
        }
        Command::Prove {
            task,
            member,
            revision,
        } => prove(estate, (task, member, revision), output).await,
        Command::Landing { command } => landing::run(estate, command, output).await,
        Command::Reference { command } => reference::run(estate, command, output).await,
        Command::Release {
            task,
            member,
            revision,
            apply,
        } => release(estate, (task, member, revision, apply), output).await,
        Command::Retire {
            task,
            member,
            artifacts,
            revision,
            apply,
        } => retire(estate, (task, member, artifacts, revision, apply), output).await,
    }
}

async fn list(
    plane: &Estate,
    task: Option<String>,
    issue: Option<String>,
    output: bool,
) -> Result<()> {
    if let Some(issue) = issue {
        let coordinate = Coordinate::parse(&issue)?;
        let node = plane.issue(&coordinate).await?.node;
        let mut members = plane.issue_worktrees().await?;
        members.retain(|member| member.node == node);
        return emit(json!({"members": members}), output);
    }
    let mut members = plane.worktrees().await?;
    if let Some(task) = task {
        members.retain(|member| member.task == task);
    }
    emit(json!({"members": members}), output)
}

async fn prove(state: &Estate, work: (String, String, i64), output: bool) -> Result<()> {
    let (subject, member, revision) = work;
    if issue(&subject) {
        let request = IssueProving {
            issue: Coordinate::parse(&subject)?,
            member,
            revision,
        };
        let member = state.prove_issue(&request).await?;
        return emit(json!({"member": member}), output);
    }
    let member = state
        .prove(&Proving {
            task: subject,
            member,
            revision,
        })
        .await?;
    emit(json!({"member": member}), output)
}

async fn release(plane: &Estate, work: (String, String, i64, bool), output: bool) -> Result<()> {
    let (subject, member, revision, apply) = work;
    explicit(apply, "member release")?;
    let revision = if issue(&subject) {
        plane
            .release_issue(&IssueRelease {
                issue: Coordinate::parse(&subject)?,
                member,
                revision,
            })
            .await?
    } else {
        plane
            .release(&Release {
                task: subject,
                member,
                revision,
            })
            .await?
    };
    emit(json!({"revision": revision}), output)
}

async fn retire(
    state: &Estate,
    work: (String, String, Vec<String>, i64, bool),
    output: bool,
) -> Result<()> {
    let (subject, member, artifacts, revision, apply) = work;
    explicit(apply, "member retire")?;
    let revision = if issue(&subject) {
        state
            .retire_issue(&IssueRetirement {
                issue: Coordinate::parse(&subject)?,
                member,
                artifacts,
                revision,
            })
            .await?
    } else {
        state
            .retire(&Retirement {
                task: subject,
                member,
                artifacts,
                revision,
            })
            .await?
    };
    emit(json!({"revision": revision}), output)
}

fn changed(change: MemberChange, output: bool) -> Result<()> {
    if !output {
        super::output::observations(&change.observations);
    }
    emit(
        json!({"member": change.member, "observations": change.observations}),
        output,
    )
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

pub(super) fn issue(value: &str) -> bool {
    value.contains('#')
}
