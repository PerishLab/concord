use super::super::{emit, explicit};
use crate::args::member::Reference;
use concord_core::{
    Coordinate, Estate, ForgeDeclaration, ForgeWithdrawal, IssueDeclaration, IssueWithdrawal,
    Result,
};
use serde_json::json;

pub(super) async fn run(estate: &Estate, command: Reference, output: bool) -> Result<()> {
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
            set(
                estate,
                (task, member, provider, owner, repository, number, revision),
                output,
            )
            .await
        }
        Reference::Remove {
            task,
            member,
            owner,
            repository,
            number,
            revision,
            apply,
        } => {
            remove(
                estate,
                (task, member, owner, repository, number, revision, apply),
                output,
            )
            .await
        }
    }
}

async fn set(
    estate: &Estate,
    reference: (String, String, String, String, String, i64, i64),
    output: bool,
) -> Result<()> {
    let (task, member, provider, owner, repository, number, revision) = reference;
    let revision = if super::issue(&task) {
        estate
            .refer_issue(&IssueDeclaration {
                issue: Coordinate::parse(&task)?,
                member,
                provider,
                owner,
                repository,
                number,
                revision,
            })
            .await?
    } else {
        estate
            .refer(&ForgeDeclaration {
                task,
                member: Some(member),
                provider,
                owner,
                repository,
                number,
                revision,
            })
            .await?
    };
    emit(json!({"revision": revision}), output)
}

async fn remove(
    estate: &Estate,
    reference: (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        i64,
        bool,
    ),
    output: bool,
) -> Result<()> {
    let (task, member, owner, repository, number, revision, apply) = reference;
    explicit(apply, "member reference remove")?;
    let revision = if super::issue(&task) {
        estate
            .unrefer_issue(&IssueWithdrawal {
                issue: Coordinate::parse(&task)?,
                member,
                owner: required(owner, "--owner")?,
                repository: required(repository, "--repository")?,
                number: required(number, "--number")?,
                revision,
            })
            .await?
    } else {
        estate
            .unrefer(&ForgeWithdrawal {
                task,
                member: Some(member),
                revision,
            })
            .await?
    };
    emit(json!({"revision": revision}), output)
}

fn required<T>(value: Option<T>, name: &str) -> Result<T> {
    value.ok_or_else(|| {
        concord_core::Error::typed(
            "concord.reference.coordinate",
            format!("Issue Member reference removal requires {name}"),
        )
    })
}
