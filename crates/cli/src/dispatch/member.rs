use super::provider;
use super::{emit, explicit};
use crate::args::Observe;
use crate::args::member::{Command, Reference};
use concord_core::{
    Attach, Claiming, Estate, ForgeDeclaration, ForgeWithdrawal, MemberChange, Narrowing, Proving,
    Release, Result, Retirement,
};
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
        Command::Status {
            task,
            member,
            observation,
        } => status(estate, (&task, &member), observation, output).await,
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
            changed(estate.attach(&request).await?, output)
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
            changed(estate.claim(&request).await?, output)
        }
        Command::Narrow {
            task,
            member,
            claim,
            revision,
            apply,
        } => {
            explicit(apply, "member narrow")?;
            let request = Narrowing {
                task,
                member,
                claims: claim,
                revision,
            };
            changed(estate.narrow(&request).await?, output)
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
        Command::Reference { command } => reference(estate, command, output).await,
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
        Command::Retire {
            task,
            member,
            artifacts,
            revision,
            apply,
        } => {
            explicit(apply, "member retire")?;
            let request = Retirement {
                task,
                member,
                artifacts,
                revision,
            };
            emit(json!({"revision": estate.retire(&request).await?}), output)
        }
    }
}

async fn reference(estate: &Estate, command: Reference, output: bool) -> Result<()> {
    match command {
        Reference::Set {
            task,
            member,
            provider,
            owner,
            repository,
            number,
            revision,
        } => {
            let declaration = ForgeDeclaration {
                task,
                member: Some(member),
                provider,
                owner,
                repository,
                number,
                revision,
            };
            emit(
                json!({"revision": estate.refer(&declaration).await?}),
                output,
            )
        }
        Reference::Remove {
            task,
            member,
            revision,
            apply,
        } => {
            explicit(apply, "member reference remove")?;
            let withdrawal = ForgeWithdrawal {
                task,
                member: Some(member),
                revision,
            };
            emit(
                json!({"revision": estate.unrefer(&withdrawal).await?}),
                output,
            )
        }
    }
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

async fn status(
    estate: &Estate,
    coordinate: (&str, &str),
    observation: Observe,
    output: bool,
) -> Result<()> {
    let status = estate.member_status(coordinate.0, coordinate.1).await?;
    let observed = if observation.observe {
        Some(
            provider::observe(
                status.reference.as_ref(),
                observation.command.as_deref(),
                observation.timeout,
            )
            .await,
        )
    } else {
        None
    };
    if output {
        let mut body = json!({"status": status});
        if let Some(observed) = &observed {
            body["observation"] = json!(observed);
        }
        return emit(body, true);
    }
    super::output::member_status(&status);
    if let Some(observed) = &observed {
        provider::print(observed);
    }
    Ok(())
}
