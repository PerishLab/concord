use super::{emit, explicit};
use crate::args::artifact::Command;
use concord_core::{Coordinate, Estate, Import, IssueImport, IssueRemoval, Removal, Result};
use serde_json::json;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { task } => list(estate, task, output).await,
        Command::Show { task, name } => show(estate, task, name, output).await,
        Command::Preflight {
            task,
            name,
            source,
            revision,
        } => preflight(estate, (task, name, source, revision), output).await,
        Command::Import {
            task,
            name,
            source,
            revision,
        } => import(estate, (task, name, source, revision), output).await,
        Command::Remove {
            task,
            name,
            revision,
            apply,
        } => remove(estate, (task, name, revision, apply), output).await,
    }
}

async fn list(plane: &Estate, subject: String, output: bool) -> Result<()> {
    if issue(&subject) {
        let issue = Coordinate::parse(&subject)?;
        let artifacts = plane.issue_artifacts(&issue).await?;
        return emit(json!({"issue": issue, "artifacts": artifacts}), output);
    }
    let artifacts = plane.artifacts(&subject).await?;
    emit(json!({"task": subject, "artifacts": artifacts}), output)
}

async fn show(state: &Estate, subject: String, name: String, output: bool) -> Result<()> {
    if issue(&subject) {
        let artifact = state
            .issue_artifact(&Coordinate::parse(&subject)?, &name)
            .await?;
        return emit(json!({"artifact": artifact}), output);
    }
    let artifact = state.artifact(&subject, &name).await?;
    emit(json!({"artifact": artifact}), output)
}

async fn preflight(
    plane: &Estate,
    request: (String, String, std::path::PathBuf, Option<i64>),
    output: bool,
) -> Result<()> {
    let (subject, name, source, revision) = request;
    if issue(&subject) {
        let request = IssueImport {
            issue: Coordinate::parse(&subject)?,
            name,
            source,
            revision: required(revision)?,
        };
        let survey = plane.preflight_issue(&request).await?;
        return emit(json!({"preflight": survey}), output);
    }
    let survey = plane
        .preflight(&Import {
            task: subject,
            name,
            source,
        })
        .await?;
    emit(json!({"preflight": survey}), output)
}

async fn import(
    state: &Estate,
    request: (String, String, std::path::PathBuf, Option<i64>),
    output: bool,
) -> Result<()> {
    let (subject, name, source, revision) = request;
    if issue(&subject) {
        let request = IssueImport {
            issue: Coordinate::parse(&subject)?,
            name,
            source,
            revision: required(revision)?,
        };
        let artifact = state.import_issue(&request).await?;
        return emit(json!({"artifact": artifact}), output);
    }
    let artifact = state
        .import(&Import {
            task: subject,
            name,
            source,
        })
        .await?;
    emit(json!({"artifact": artifact}), output)
}

async fn remove(
    plane: &Estate,
    request: (String, String, Option<i64>, bool),
    output: bool,
) -> Result<()> {
    let (subject, name, revision, apply) = request;
    explicit(apply, "artifact remove")?;
    if issue(&subject) {
        let revision = plane
            .remove_issue(&IssueRemoval {
                issue: Coordinate::parse(&subject)?,
                name,
                revision: required(revision)?,
            })
            .await?;
        return emit(json!({"removed": true, "revision": revision}), output);
    }
    plane
        .remove(&Removal {
            task: subject,
            name,
        })
        .await?;
    emit(json!({"removed": true}), output)
}

fn issue(value: &str) -> bool {
    value.contains('#')
}

fn required(revision: Option<i64>) -> Result<i64> {
    revision.ok_or_else(|| {
        concord_core::Error::typed(
            "concord.issue.revision",
            "Issue Artifact writes require --revision",
        )
    })
}
