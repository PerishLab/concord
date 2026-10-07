use concord_core::Result;
use concord_core::acceptance::{Marker, Plan};
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::forge;
use super::plan::Prepared;
use crate::args::acceptance::Prepare;

const SHAPE: &str = r#"{"prose":"Concrete acceptance promise","marker":{"purpose":"declaration","target":"source","promise":"Merge the bounded verified source change."}}"#;

pub(super) enum Purpose {
    Declaration,
    Amendment,
    Closure,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    prose: String,
    marker: Marker,
}

pub(super) async fn run(args: Prepare, purpose: Purpose) -> Result<Value> {
    let intent: Intent = crate::dispatch::input::read(&args.input, SHAPE)?;
    let execution = forge::execution()?;
    validate(&intent, purpose)?;
    let prose = format!("{}\n\n{}", intent.prose.trim_end(), intent.marker.render()?);
    let body = forge::render(&prose, &execution)?;
    let snapshot = super::observe(&args.observe).await?;
    if snapshot.header.state != "OPEN" {
        return Err(super::fault(
            "closed",
            "acceptance preparation requires an open Issue",
        ));
    }
    let plan = Plan::prepare(intent.marker, body, &snapshot.labels, &snapshot.comments())?;
    let prepared = Prepared {
        schema: "concord.acceptance-plan/v1".into(),
        review: snapshot.review()?,
        issue: snapshot.header,
        intent: plan,
        execution,
        semantics: "prepared attestation only; no provider write, readiness proof or authenticated identity".into(),
    };
    Ok(json!({"plan": prepared}))
}

fn validate(intent: &Intent, purpose: Purpose) -> Result<()> {
    if intent.prose.trim().is_empty() {
        return Err(super::fault(
            "prose",
            "acceptance preparation requires explanatory prose",
        ));
    }
    let agrees = matches!(
        (&intent.marker, purpose),
        (Marker::Declaration { .. }, Purpose::Declaration)
            | (Marker::Amendment { .. }, Purpose::Amendment)
            | (Marker::Closure { .. }, Purpose::Closure)
    );
    if !agrees {
        return Err(super::fault(
            "purpose",
            "input marker purpose differs from the selected operation",
        ));
    }
    Ok(())
}
