use super::{emit, explicit};
use crate::args::domain::{Command, Repository as Action};
use concord_core::{Annotate, Estate, Result, Retire};
use serde_json::json;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::Bootstrap { .. } => unreachable!("handled before estate open"),
        Command::List => emit(json!({"domains": estate.realms().await?}), output),
        Command::Add { name } => {
            let key = estate.manage(&name).await?;
            emit(value(key, name), output)
        }
        Command::Repository { command } => repository(estate, command, output).await,
    }
}

async fn repository(estate: &Estate, command: Action, output: bool) -> Result<()> {
    match command {
        Action::List { domain, retired } => emit(
            json!({"repositories": estate.repositories(domain.as_deref(), retired).await?}),
            output,
        ),
        Action::Annotate {
            domain,
            name,
            note,
            revision,
        } => {
            let (repository, revision) = estate
                .annotate(&Annotate {
                    domain,
                    name,
                    note,
                    revision,
                })
                .await?;
            emit(
                json!({"repository": repository, "revision": revision}),
                output,
            )
        }
        Action::Retire {
            domain,
            name,
            revision,
            reason,
            apply,
        } => {
            explicit(apply, "domain repository retire")?;
            let (repository, revision) = estate
                .tombstone(&Retire {
                    domain,
                    name,
                    reason,
                    revision,
                })
                .await?;
            emit(
                json!({"repository": repository, "revision": revision}),
                output,
            )
        }
    }
}

pub fn value(key: i64, name: String) -> serde_json::Value {
    json!({"domain": {"key": key, "name": name, "revision": 0}})
}
