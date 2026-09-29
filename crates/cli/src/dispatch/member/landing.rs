use super::super::{emit, input};
use crate::args::member::Landing;
use concord_core::authority::Plumb;
use concord_core::{Coordinate, Estate, Result, issue_landing};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u64,
    plan: issue_landing::Plan,
}

pub(super) async fn run(estate: &Estate, command: Landing, output: bool) -> Result<()> {
    match command {
        Landing::Prepare {
            issue,
            base,
            title,
            body,
            guard_schema,
            guard_tree,
            guard_digest,
            revision,
        } => {
            let request = issue_landing::Request {
                issue: Coordinate::parse(&issue)?,
                revision,
                base,
                title,
                body,
                guard: issue_landing::Guard {
                    schema: guard_schema,
                    tree: guard_tree,
                    digest: guard_digest,
                },
            };
            emit(
                json!({"plan": issue_landing::prepare::<Plumb>(estate, &request).await?}),
                output,
            )
        }
        Landing::Ready { issue, plan } => {
            let envelope: Envelope = input::read(&plan, PLAN)?;
            if envelope.version != 1 || envelope.plan.issue != Coordinate::parse(&issue)? {
                return Err(concord_core::Error::typed(
                    "concord.landing.coordinate",
                    "landing plan version, Issue, or Member does not match the command",
                ));
            }
            emit(
                json!({"ready": issue_landing::revalidate::<Plumb>(estate, &envelope.plan).await?}),
                output,
            )
        }
    }
}

const PLAN: &str = r#"{"version":1,"plan":{"schema":"concord.issue-member-landing/v3","issue":{"owner":"OWNER","repository":"REPOSITORY","number":1},"node":"I_node","revision":1,"member":{},"boundary":{},"guard":{},"authority":{},"landing":{}}}"#;
