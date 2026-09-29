use super::super::{emit, explicit};
use crate::args::member::Reference;
use concord_core::{Coordinate, Estate, IssueDeclaration, IssueWithdrawal, Result};
use serde_json::json;

pub(super) async fn run(estate: &Estate, command: Reference, output: bool) -> Result<()> {
    let revision = match command {
        Reference::Set {
            issue,
            provider,
            owner,
            repository,
            number,
            revision,
        } => {
            estate
                .refer_issue(&IssueDeclaration {
                    issue: Coordinate::parse(&issue)?,
                    provider,
                    owner,
                    repository,
                    number,
                    revision,
                })
                .await?
        }
        Reference::Remove {
            issue,
            owner,
            repository,
            number,
            revision,
            apply,
        } => {
            explicit(apply, "member reference remove")?;
            estate
                .unrefer_issue(&IssueWithdrawal {
                    issue: Coordinate::parse(&issue)?,
                    owner,
                    repository,
                    number,
                    revision,
                })
                .await?
        }
    };
    emit(json!({"revision": revision}), output)
}
