use concord_core::acceptance::{Comment, Marker, Plan, Step, Target};
use std::slice::from_ref;

use crate::{closure, comment, declaration, reference};

fn prepare(marker: Marker, labels: &[String], comments: &[Comment]) -> Plan {
    let body = comment("intended", &marker).body;
    Plan::prepare(marker, body, labels, comments).unwrap()
}

fn labels(target: Target) -> Vec<String> {
    vec!["priority:normal".into(), target.label().into()]
}

fn amendment(root: &Comment) -> Marker {
    Marker::Amendment {
        target: Target::Release,
        promise: "Deliver and verify the installed stable capability.".into(),
        predecessor: reference(root),
        reason: "Source delivery alone no longer satisfies this Issue.".into(),
    }
}

#[test]
fn partial() {
    let marker = declaration(Target::Source);
    let intended = comment("published", &marker);
    let plan = prepare(marker, &[], &[]);
    let pending = plan.recover(&[], &[]).unwrap();
    assert_eq!(pending.step, Step::Publish);
    assert_eq!(pending.reference, None);
    let retained = plan.recover(&[], from_ref(&intended)).unwrap();
    assert_eq!(retained.step, Step::Relabel);
    assert_eq!(retained.reference, Some(reference(&intended)));
    let complete = plan.recover(&labels(Target::Source), &[intended]).unwrap();
    assert_eq!(complete.step, Step::Complete);
    assert_eq!(complete.target, Target::Source);
}

#[test]
fn premature() {
    let plan = prepare(declaration(Target::Source), &[], &[]);
    assert_eq!(
        plan.recover(&labels(Target::Source), &[]).unwrap().step,
        Step::Publish
    );
}

#[test]
fn transition() {
    let root = comment("root", &declaration(Target::Source));
    let marker = amendment(&root);
    let intended = comment("amended", &marker);
    let plan = prepare(marker, &labels(Target::Source), from_ref(&root));
    assert_eq!(
        plan.recover(&labels(Target::Release), from_ref(&root))
            .unwrap()
            .step,
        Step::Publish
    );
    let history = vec![root, intended];
    assert_eq!(plan.recover(&[], &history).unwrap().step, Step::Relabel);
    assert_eq!(
        plan.recover(&labels(Target::Source), &history)
            .unwrap()
            .step,
        Step::Relabel
    );
    assert_eq!(
        plan.recover(&labels(Target::Release), &history)
            .unwrap()
            .step,
        Step::Complete
    );
}

#[test]
fn reversal() {
    let root = comment("root", &crate::declaration(Target::Release));
    let marker = Marker::Amendment {
        target: Target::Source,
        promise: "Merge the verified source change.".into(),
        predecessor: reference(&root),
        reason: "Installed release verification is a separate Issue.".into(),
    };
    let intended = comment("amended", &marker);
    let plan = prepare(marker, &labels(Target::Release), from_ref(&root));
    let history = vec![root, intended];
    assert_eq!(
        plan.recover(&labels(Target::Release), &history)
            .unwrap()
            .step,
        Step::Relabel
    );
    assert_eq!(
        plan.recover(&labels(Target::Source), &history)
            .unwrap()
            .step,
        Step::Complete
    );
}

#[test]
fn retention() {
    let root = comment("root", &declaration(Target::Source));
    let marker = closure(&root, Target::Source);
    let intended = comment("closed", &marker);
    let plan = prepare(marker, &labels(Target::Source), from_ref(&root));
    let result = plan
        .recover(&labels(Target::Source), &[root, intended.clone()])
        .unwrap();
    assert_eq!(result.step, Step::Complete);
    assert_eq!(result.reference, Some(reference(&intended)));
}

#[test]
fn ordinary() {
    let plan = prepare(declaration(Target::Source), &[], &[]);
    let ordinary = Comment {
        node: "discussion".into(),
        body: "Independent progress observation.".into(),
    };
    let labels = vec!["needs:review".into(), "priority:urgent".into()];
    assert_eq!(
        plan.recover(&labels, from_ref(&ordinary)).unwrap().step,
        Step::Publish
    );
    let intended = Comment {
        node: "published".into(),
        body: plan.body().unwrap().into(),
    };
    assert_eq!(
        plan.recover(&labels, &[intended, ordinary]).unwrap().step,
        Step::Relabel
    );
    assert_eq!(labels, ["needs:review", "priority:urgent"]);
}

#[test]
fn stale() {
    let root = comment("root", &declaration(Target::Source));
    let plan = prepare(amendment(&root), &labels(Target::Source), from_ref(&root));
    let mut edited = root.clone();
    edited
        .body
        .push_str("\nEdited prose changes its exact reference.");
    assert!(plan.recover(&labels(Target::Source), &[edited]).is_err());
    assert!(plan.recover(&labels(Target::Source), &[]).is_err());
    let newer = comment("newer", &closure(&root, Target::Source));
    assert!(
        plan.recover(&labels(Target::Source), &[root, newer])
            .is_err()
    );
}

#[test]
fn later() {
    let marker = declaration(Target::Source);
    let intended = comment("published", &marker);
    let plan = prepare(marker, &[], &[]);
    let newer = comment("newer", &amendment(&intended));
    assert!(
        plan.recover(&labels(Target::Release), &[intended, newer])
            .is_err()
    );
}

#[test]
fn edited() {
    let marker = declaration(Target::Source);
    let mut intended = comment("published", &marker);
    let plan = prepare(marker, &[], &[]);
    intended.body.push_str("\nProvider-side edit.");
    assert!(plan.recover(&labels(Target::Source), &[intended]).is_err());
}

#[test]
fn duplicate() {
    let root = comment("root", &declaration(Target::Source));
    let marker = closure(&root, Target::Source);
    let plan = prepare(marker.clone(), &labels(Target::Source), from_ref(&root));
    let one = comment("one", &marker);
    let two = comment("two", &marker);
    assert!(
        plan.recover(&labels(Target::Source), &[root, one, two])
            .is_err()
    );
}

#[test]
fn conflicts() {
    let plan = prepare(declaration(Target::Source), &[], &[]);
    assert!(plan.recover(&labels(Target::Release), &[]).is_err());
    let mut conflict = labels(Target::Source);
    conflict.push(Target::Release.label().into());
    assert!(plan.recover(&conflict, &[]).is_err());
    assert!(plan.recover(&["acceptance:unknown".into()], &[]).is_err());
    let root = comment("root", &declaration(Target::Source));
    let plan = prepare(amendment(&root), &labels(Target::Source), from_ref(&root));
    assert!(plan.recover(&[], &[root]).is_err());
}

#[test]
fn persistence() {
    let plan = prepare(declaration(Target::Source), &[], &[]);
    let encoded = serde_json::to_value(&plan).unwrap();
    let restored: Plan = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored, plan);
    assert_eq!(restored.recover(&[], &[]).unwrap().step, Step::Publish);
    let mut changed = encoded.clone();
    changed["basis"] = serde_json::json!([{
        "node": "invented",
        "digest": concord_core::acceptance::digest("invented"),
    }]);
    let changed: Plan = serde_json::from_value(changed).unwrap();
    assert!(changed.recover(&[], &[]).is_err());
    let mut changed = encoded.clone();
    changed["body"] = "No acceptance marker.".into();
    let changed: Plan = serde_json::from_value(changed).unwrap();
    assert!(changed.recover(&[], &[]).is_err());
    let mut changed = encoded.clone();
    changed["prior"] = "release".into();
    let changed: Plan = serde_json::from_value(changed).unwrap();
    assert!(changed.recover(&[], &[]).is_err());
    let mut changed = encoded;
    changed["authority"] = true.into();
    assert!(serde_json::from_value::<Plan>(changed).is_err());
}

#[test]
fn preparation() {
    let marker = declaration(Target::Source);
    let body = comment("intended", &marker).body;
    assert!(Plan::prepare(marker.clone(), body.clone(), &labels(Target::Source), &[]).is_err());
    let other = comment("other", &declaration(Target::Release)).body;
    assert!(Plan::prepare(marker.clone(), other, &[], &[]).is_err());
    let root = comment("root", &marker);
    let closed = closure(&root, Target::Source);
    let prior = comment("closed", &closed);
    let body = prior.body.clone();
    assert!(Plan::prepare(closed, body, &labels(Target::Source), &[root, prior]).is_err());
    let malformed = Comment {
        node: "malformed".into(),
        body: "<!-- concord.acceptance/v99\n{}\n-->".into(),
    };
    assert!(Plan::prepare(marker, prose(), &[], &[malformed]).is_err());
}

fn prose() -> String {
    comment("intended", &declaration(Target::Source)).body
}
