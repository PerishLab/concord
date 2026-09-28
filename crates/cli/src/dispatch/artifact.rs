use super::{emit, explicit};
use crate::args::artifact::Command;
use concord_core::{Coordinate, Estate, IssueImport, IssueRemoval, Result};
use serde_json::json;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { issue } => {
            let issue = Coordinate::parse(&issue)?;
            emit(
                json!({"issue": issue, "artifacts": estate.issue_artifacts(&issue).await?}),
                output,
            )
        }
        Command::Show { issue, name } => emit(
            json!({"artifact": estate.issue_artifact(&Coordinate::parse(&issue)?, &name).await?}),
            output,
        ),
        Command::Preflight {
            issue,
            name,
            source,
            revision,
        } => {
            let request = IssueImport {
                issue: Coordinate::parse(&issue)?,
                name,
                source,
                revision,
            };
            emit(
                json!({"preflight": estate.preflight_issue(&request).await?}),
                output,
            )
        }
        Command::Import {
            issue,
            name,
            source,
            revision,
        } => {
            let request = IssueImport {
                issue: Coordinate::parse(&issue)?,
                name,
                source,
                revision,
            };
            emit(
                json!({"artifact": estate.import_issue(&request).await?}),
                output,
            )
        }
        Command::Remove {
            issue,
            name,
            revision,
            apply,
        } => {
            explicit(apply, "artifact remove")?;
            let revision = estate
                .remove_issue(&IssueRemoval {
                    issue: Coordinate::parse(&issue)?,
                    name,
                    revision,
                })
                .await?;
            emit(json!({"removed": true, "revision": revision}), output)
        }
    }
}
