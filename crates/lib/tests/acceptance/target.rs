use concord_core::acceptance::{
    Comment, Evaluation, Fact, History, Marker, Observation, Plan, Step, Target, Verdict,
};
use serde_json::{Value, json};
use std::slice::from_ref;

use crate::{comment, declaration, reference};

const CODE: &str = "concord.acceptance.target";

fn labels(target: Target) -> Vec<String> {
    vec!["priority:normal".into(), target.label().into()]
}

fn refusal(result: concord_core::Result<impl std::fmt::Debug>) -> (String, Value) {
    let error = result.unwrap_err();
    assert_eq!(error.code(), CODE, "{}", error.message());
    (error.message().into(), error.details().cloned().unwrap())
}

fn prepare(marker: &Marker, labels: &[String], comments: &[Comment]) -> concord_core::Result<Plan> {
    let body = comment("intended", marker).body;
    Plan::prepare(marker.clone(), body, labels, comments)
}

fn evaluate(labels: &[String], comments: &[Comment]) -> concord_core::Result<Evaluation> {
    let fact = Fact {
        verdict: Verdict::Unknown,
        reason: "not observed".into(),
    };
    Evaluation::read(Observation {
        labels,
        comments,
        body: "Current exact acceptance conditions.",
        freshness: fact.clone(),
        conditions: Vec::new(),
        release: fact,
    })
}

#[test]
fn orphan() {
    let source = labels(Target::Source);
    let (message, details) = refusal(History::default().target(&source));
    assert!(
        message
            .contains("acceptance:source is present but the Issue has no acceptance declaration")
    );
    assert!(message.contains("declare target source to adopt the label"));
    assert_eq!(
        details,
        json!({"label": "acceptance:source", "declared": null, "declaration": null})
    );
    refusal(evaluate(&source, &[]));
    let marker = declaration(Target::Source);
    let plan = prepare(&marker, &source, &[]).unwrap();
    assert_eq!(plan.recover(&source, &[]).unwrap().step, Step::Publish);
    let intended = comment("intended", &marker);
    let adopted = plan.recover(&source, from_ref(&intended)).unwrap();
    assert_eq!(adopted.step, Step::Complete);
    assert_eq!(adopted.reference, Some(reference(&intended)));
    let history = History::read(from_ref(&intended)).unwrap();
    assert_eq!(history.target(&source).unwrap(), Some(Target::Source));
    let (message, details) = refusal(prepare(&declaration(Target::Release), &source, &[]));
    assert!(message.contains("or remove acceptance:source before declaring another target"));
    assert_eq!(details["label"], "acceptance:source");
}

#[test]
fn disagreement() {
    let root = comment("IC_root", &declaration(Target::Source));
    let release = labels(Target::Release);
    let (message, details) = refusal(History::read(from_ref(&root)).unwrap().target(&release));
    assert!(message.contains("acceptance:release disagrees with current declaration IC_root"));
    assert!(message.contains("replace acceptance:release with acceptance:source"));
    assert_eq!(
        details,
        json!({"label": "acceptance:release", "declared": "acceptance:source",
            "declaration": reference(&root)})
    );
    refusal(evaluate(&release, from_ref(&root)));
    refusal(prepare(
        &declaration(Target::Release),
        &release,
        from_ref(&root),
    ));
    let amendment = Marker::Amendment {
        target: Target::Release,
        promise: "Deliver and verify the installed stable capability.".into(),
        predecessor: reference(&root),
        reason: "Source delivery alone no longer satisfies this Issue.".into(),
    };
    refusal(prepare(&amendment, &release, from_ref(&root)));
}

#[test]
fn unlabelled() {
    let root = comment("IC_root", &declaration(Target::Source));
    let bare = vec!["priority:normal".to_string()];
    let (message, details) = refusal(History::read(from_ref(&root)).unwrap().target(&bare));
    assert!(message.contains("current declaration IC_root targets source but no managed label"));
    assert!(message.contains("re-run acceptance apply with the plan that published IC_root"));
    assert_eq!(details["label"], Value::Null);
    assert_eq!(details["declared"], "acceptance:source");
    refusal(evaluate(&bare, from_ref(&root)));
}
