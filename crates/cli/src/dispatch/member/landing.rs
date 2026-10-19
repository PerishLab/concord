use super::super::{emit, input};
use crate::args::member::Landing;
use concord_core::authority::Plumb;
use concord_core::{Coordinate, Estate, Result, landing};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u64,
    plan: landing::Plan,
}

pub(super) async fn run(estate: &Estate, command: Landing, output: bool) -> Result<()> {
    match command {
        Landing::Prepare {
            issue,
            base,
            title,
            body,
            schema,
            tree,
            digest,
            revision,
        } => {
            let request = landing::Request {
                issue: Coordinate::parse(&issue)?,
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
            emit(
                json!({"plan": landing::prepare::<Plumb>(estate, &request).await?}),
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
                json!({"ready": landing::revalidate::<Plumb>(estate, &envelope.plan).await?}),
                output,
            )
        }
    }
}

const PLAN: &str = r#"{"version":1,"plan":{"schema":"concord.issue-member-landing/v3","issue":{"owner":"OWNER","repository":"REPOSITORY","number":1},"node":"I_node","revision":1,"member":{},"boundary":{},"guard":{},"authority":{},"landing":{}}}"#;
