use super::super::{emit, input};
use crate::args::member::Landing;
use concord_core::{Coordinate, Estate, Result, issue_landing, landing};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u64,
    plan: landing::Plan,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnchorEnvelope {
    version: u64,
    plan: issue_landing::Plan,
}

pub(super) async fn run(plane: &Estate, command: Landing, output: bool) -> Result<()> {
    match command {
        Landing::Prepare {
            task,
            member,
            base,
            title,
            body,
            guard_schema,
            guard_tree,
            guard_digest,
            revision,
        } => {
            let guard = (guard_schema, guard_tree, guard_digest);
            let work = (task, member, base, title, body, revision);
            if super::issue(&work.0) {
                prepare_anchor(plane, work, guard, output).await
            } else {
                prepare_task(plane, work, guard, output).await
            }
        }
        Landing::Ready { task, member, plan } => ready(plane, (task, member, plan), output).await,
    }
}

async fn prepare_anchor(
    state: &Estate,
    work: (String, String, String, String, String, i64),
    guard: (String, String, String),
    output: bool,
) -> Result<()> {
    let (task, member, base, title, body, revision) = work;
    let (schema, tree, digest) = guard;
    let request = issue_landing::Request {
        issue: Coordinate::parse(&task)?,
        member,
        revision,
        base,
        title,
        body,
        guard: issue_landing::Guard {
            schema,
            tree,
            digest,
        },
    };
    let plan = issue_landing::prepare(state, &request).await?;
    emit(json!({"plan": plan}), output)
}

async fn prepare_task(
    estate: &Estate,
    work: (String, String, String, String, String, i64),
    guard: (String, String, String),
    output: bool,
) -> Result<()> {
    let (task, member, base, title, body, revision) = work;
    let (schema, tree, digest) = guard;
    let request = landing::Request {
        task,
        member,
        revision,
        base,
        title,
        body,
        guard: landing::Guard {
            schema,
            tree,
            digest,
        },
    };
    let plan = landing::prepare(estate, &request).await?;
    emit(json!({"plan": plan}), output)
}

async fn ready(
    plane: &Estate,
    request: (String, String, std::path::PathBuf),
    output: bool,
) -> Result<()> {
    if super::issue(&request.0) {
        return ready_anchor(plane, request, output).await;
    }
    ready_task(plane, request, output).await
}

async fn ready_anchor(
    state: &Estate,
    request: (String, String, std::path::PathBuf),
    output: bool,
) -> Result<()> {
    let (task, member, plan) = request;
    let envelope: AnchorEnvelope = input::read(&plan, ANCHOR_PLAN)?;
    if envelope.version != 1
        || envelope.plan.issue != Coordinate::parse(&task)?
        || envelope.plan.member.name != member
    {
        return Err(coordinate("Issue"));
    }
    let ready = issue_landing::revalidate(state, &envelope.plan).await?;
    emit(json!({"ready": ready}), output)
}

async fn ready_task(
    estate: &Estate,
    request: (String, String, std::path::PathBuf),
    output: bool,
) -> Result<()> {
    let (task, member, plan) = request;
    let envelope: Envelope = input::read(&plan, PLAN)?;
    if envelope.version != 1 || envelope.plan.task != task || envelope.plan.member.name != member {
        return Err(coordinate("Task"));
    }
    let ready = landing::revalidate(estate, &envelope.plan).await?;
    emit(json!({"ready": ready}), output)
}

fn coordinate(kind: &str) -> concord_core::Error {
    concord_core::Error::typed(
        "concord.landing.coordinate",
        format!("landing plan version, {kind}, or Member does not match the command"),
    )
}

const PLAN: &str = r#"{"version":1,"plan":{"schema":"concord.member-landing/v1","task":"DOMAIN/TASK","revision":1,"member":{},"boundary":{},"guard":{},"landing":{}}}"#;
const ANCHOR_PLAN: &str = r#"{"version":1,"plan":{"schema":"concord.issue-member-landing/v1","issue":{"owner":"OWNER","repository":"REPOSITORY","number":1},"node":"I_node","revision":1,"member":{},"boundary":{},"guard":{},"landing":{}}}"#;
