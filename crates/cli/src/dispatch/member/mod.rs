use super::{emit, explicit};
use crate::args::member::Command;
use concord_core::{
    Change, Claiming, Coordinate, Estate, Finish, Narrowing, Proving, Recovery, Release, Result,
    Retirement, Start,
};
use serde_json::json;

mod landing;
mod reference;
mod start;
mod status;

pub(super) fn supported(command: &Command) -> Result<()> {
    if let Command::Landing { command } = command {
        landing::run(command)?;
    }
    Ok(())
}

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { issue } => list(estate, issue, output).await,
        Command::Status { issue } => status::run(estate, &issue, output).await,
        Command::Start {
            issue,
            claim,
            revision,
            timeout,
        } => {
            let issue = Coordinate::parse(&issue)?;
            let observed = start::observe(&issue, timeout).await?;
            changed(
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
        } => changed(
            estate
                .claim(&Claiming {
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
            changed(
                estate
                    .narrow(&Narrowing {
                        issue: Coordinate::parse(&issue)?,
                        claims: claim,
                        revision,
                    })
                    .await?,
                output,
            )
        }
        Command::Prove { issue, revision } => prove(estate, &issue, revision, output).await,
        Command::Landing { command } => landing::run(&command),
        Command::Reference { command } => reference::run(estate, command, output).await,
        Command::Release {
            issue,
            revision,
            apply,
        } => {
            explicit(apply, "member release")?;
            let revision = estate
                .release(&Release {
                    issue: Coordinate::parse(&issue)?,
                    revision,
                })
                .await?;
            emit(json!({"revision": revision}), output)
        }
        Command::Recover {
            issue,
            revision,
            head,
            apply,
        } => {
            explicit(apply, "member recover")?;
            let revision = estate
                .recover(&Recovery {
                    issue: Coordinate::parse(&issue)?,
                    revision,
                    head,
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
                .dismiss(&Retirement {
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
    let mut members = estate.worktrees().await?;
    if let Some(issue) = issue {
        let node = estate.issue(&Coordinate::parse(&issue)?).await?.node;
        members.retain(|member| member.node == node);
    }
    emit(json!({"members": members}), output)
}

fn changed(change: Change, output: bool) -> Result<()> {
    if !output {
        super::output::observations(&change.observations);
    }
    emit(
        json!({"member": change.member, "observations": change.observations}),
        output,
    )
}

async fn prove(estate: &Estate, issue: &str, revision: i64, output: bool) -> Result<()> {
    let coordinate = Coordinate::parse(issue)?;
    let member = estate
        .prove(&Proving {
            issue: coordinate.clone(),
            revision,
        })
        .await?;
    let advisory = match estate.status(&coordinate).await.ok() {
        Some(status) => stale(&status).await,
        None => None,
    };
    let Some(note) = advisory else {
        return emit(json!({"member": member}), output);
    };
    if !output {
        eprintln!("concord: advisory: {note}");
    }
    emit(json!({"member": member, "advisory": note}), output)
}

async fn stale(status: &concord_core::Status) -> Option<String> {
    let remote = &status.member.integration.remote;
    let branch = &status.member.integration.branch;
    let tracking = format!("refs/remotes/{remote}/{branch}");
    let head = tokio::process::Command::new("git")
        .args(["rev-parse", "--verify", &tracking])
        .current_dir(&status.worktree.path)
        .output()
        .await
        .ok()?;
    if !head.status.success() {
        return None;
    }
    let upstream = String::from_utf8_lossy(&head.stdout).trim().to_string();
    let check = tokio::process::Command::new("git")
        .args(["merge-base", "--is-ancestor", &upstream, "HEAD"])
        .current_dir(&status.worktree.path)
        .status()
        .await
        .ok()?;
    if check.code() != Some(1) {
        return None;
    }
    Some(format!(
        "local {remote}/{branch} ({upstream}) is ahead of Member HEAD ({}); merge {remote}/{branch} and rerun plumb guard before delivery",
        status.worktree.head
    ))
}
