use super::{emit, explicit};
use crate::args::member::Command;
use concord_core::{Attach, Claiming, Estate, Proving, Release, Result};
use serde_json::json;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { task } => {
            let mut members = estate.worktrees().await?;
            if let Some(task) = task {
                members.retain(|member| member.task == task);
            }
            emit(json!({"members": members}), output)
        }
        Command::Status { task, member } => status(estate, &task, &member, output).await,
        Command::Attach {
            task,
            name,
            source,
            branch,
            claim,
            revision,
        } => {
            let request = Attach {
                task,
                name,
                source,
                branch,
                claims: claim,
                revision,
            };
            emit(json!({"member": estate.attach(&request).await?}), output)
        }
        Command::Claim {
            task,
            member,
            claim,
            revision,
        } => {
            let request = Claiming {
                task,
                member,
                claims: claim,
                revision,
            };
            emit(json!({"member": estate.claim(&request).await?}), output)
        }
        Command::Prove {
            task,
            member,
            revision,
        } => {
            let request = Proving {
                task,
                member,
                revision,
            };
            emit(json!({"member": estate.prove(&request).await?}), output)
        }
        Command::Release {
            task,
            member,
            revision,
            apply,
        } => {
            explicit(apply, "member release")?;
            let request = Release {
                task,
                member,
                revision,
            };
            emit(json!({"revision": estate.release(&request).await?}), output)
        }
    }
}

async fn status(estate: &Estate, task: &str, member: &str, output: bool) -> Result<()> {
    let status = estate.member_status(task, member).await?;
    if output {
        return emit(json!({"status": status}), true);
    }
    super::output::member_status(&status);
    Ok(())
}
