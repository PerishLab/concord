use concord_core::Result;
use concord_core::acceptance::{Marker, Plan};
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::forge;
use super::plan::Prepared;
use crate::args::acceptance::Prepare;

mod advice;

const DECLARATION: &str = r#"{"prose":"Concrete acceptance promise","marker":{"purpose":"declaration","target":"source","promise":"Merge the bounded verified source change."}}"#;
const AMENDMENT: &str = r#"{"prose":"Explain why the acceptance promise changed","marker":{"purpose":"amendment","target":"source","promise":"REPLACE_WITH_THE_AMENDED_ACCEPTANCE_PROMISE","predecessor":{"node":"REPLACE_WITH_PROVIDER_COMMENT_NODE_OF_THE_CURRENT_DECLARATION","digest":"REPLACE_WITH_LOWERCASE_SHA256_OF_THE_DECLARATION_COMMENT_BODY"},"reason":"REPLACE_WITH_THE_REASON_FOR_THIS_AMENDMENT"}}"#;
const CLOSURE: &str = r#"{"prose":"Record the reviewed acceptance judgment; see judge --help for release shape and reviewer-choice rules","marker":{"purpose":"closure","target":"source","declaration":{"node":"REPLACE_WITH_PROVIDER_COMMENT_NODE_OF_THE_CURRENT_DECLARATION","digest":"REPLACE_WITH_LOWERCASE_SHA256_OF_THE_DECLARATION_COMMENT_BODY"},"judgment":"unmet","verification":"manual","review":"REPLACE_WITH_LOWERCASE_SHA256_OF_THE_EXACT_CURRENT_ISSUE_BODY","evidence":["REPLACE_WITH_EXACT_EVIDENCE_REFERENCE_FOR_A_SETTLED_CONDITION"],"remaining":["REPLACE_WITH_ACCEPTANCE_OBLIGATION_STILL_OUTSTANDING"],"release":null}}"#;

fn shape(purpose: &Purpose) -> &'static str {
    match purpose {
        Purpose::Declaration => DECLARATION,
        Purpose::Amendment => AMENDMENT,
        Purpose::Closure => CLOSURE,
    }
}

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
    let intent: Intent = crate::dispatch::input::read(&args.input, shape(&purpose))?;
    let execution = forge::execution()?;
    let advisory = matches!(&purpose, Purpose::Declaration | Purpose::Amendment)
        && intent.marker.target() == concord_core::acceptance::Target::Release;
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
    super::plan::review(&intent.marker, &snapshot.header.body)?;
    let plan = Plan::prepare(intent.marker, body, &snapshot.labels, &snapshot.comments())?;
    let authority = if advisory {
        Some(advice::read(&args.observe, &snapshot.header).await)
    } else {
        None
    };
    let prepared = Prepared {
        schema: if authority.is_some() {
            "concord.acceptance-plan/v2"
        } else {
            "concord.acceptance-plan/v1"
        }.into(),
        review: snapshot.review()?,
        issue: snapshot.header,
        intent: plan,
        execution,
        authority,
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
