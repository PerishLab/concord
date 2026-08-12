use super::{emit, explicit};
use crate::args::artifact::Command;
use concord_core::{Estate, Import, Removal, Result};
use serde_json::json;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { task } => emit(
            json!({"task": task, "artifacts": estate.artifacts(&task).await?}),
            output,
        ),
        Command::Show { task, name } => emit(
            json!({"artifact": estate.artifact(&task, &name).await?}),
            output,
        ),
        Command::Preflight { task, name, source } => {
            let request = Import { task, name, source };
            emit(
                json!({"preflight": estate.preflight(&request).await?}),
                output,
            )
        }
        Command::Import { task, name, source } => {
            let request = Import { task, name, source };
            emit(json!({"artifact": estate.import(&request).await?}), output)
        }
        Command::Remove { task, name, apply } => {
            explicit(apply, "artifact remove")?;
            estate.remove(&Removal { task, name }).await?;
            emit(json!({"removed": true}), output)
        }
    }
}
