use super::{emit, explicit};
use crate::args::artifact::{Command, Import};
use concord_core::{Coordinate, Estate, Removal, Result};
use serde_json::json;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { issue } => {
            let issue = Coordinate::parse(&issue)?;
            emit(
                json!({"issue": issue, "artifacts": estate.artifacts(&issue).await?}),
                output,
            )
        }
        Command::Show { issue, name } => emit(
            json!({"artifact": estate.artifact(&Coordinate::parse(&issue)?, &name).await?}),
            output,
        ),
        Command::Preflight(import) => emit(
            json!({"preflight": estate.preflight(&request(import)?).await?}),
            output,
        ),
        Command::Import(import) => emit(
            json!({"artifact": estate.import(&request(import)?).await?}),
            output,
        ),
        Command::Remove {
            issue,
            name,
            revision,
            apply,
        } => {
            explicit(apply, "artifact remove")?;
            let revision = estate
                .remove(&Removal {
                    issue: Coordinate::parse(&issue)?,
                    name,
                    revision,
                })
                .await?;
            emit(json!({"removed": true, "revision": revision}), output)
        }
    }
}

fn request(import: Import) -> Result<concord_core::Import> {
    let Import {
        issue,
        name,
        source,
        revision,
    } = import;
    Ok(concord_core::Import {
        issue: Coordinate::parse(&issue)?,
        name,
        source,
        revision,
    })
}
