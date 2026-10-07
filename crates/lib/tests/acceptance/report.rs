use concord_core::acceptance::{
    Check, Evaluation, Fact, Reference, Report, Target, Verdict, Verification,
};

fn report(target: Target) -> Report {
    Report {
        evaluation: Evaluation {
            target: Some(target),
            declaration: Some(Reference {
                node: "IC_declaration".into(),
                digest: "a".repeat(64),
            }),
            closure: None,
            verification: None,
            verdict: Verdict::Unknown,
            checks: vec![Check {
                name: "release".into(),
                fact: Fact {
                    verdict: Verdict::Unknown,
                    reason: "exact stable does not yet include this change".into(),
                },
            }],
        },
        promise: "Verify the installed command, not only its merge".into(),
        checklist: "- [x] source\n- [ ] containing stable and installed behavior".into(),
    }
}

#[test]
fn targets() {
    for target in [Target::Source, Target::Release] {
        let rendered = report(target).render();
        assert!(rendered.contains(target.label()));
        assert!(rendered.contains("IC_declaration"));
        assert!(rendered.contains(&"a".repeat(64)));
        assert!(rendered.contains("exact stable does not yet include"));
        assert!(rendered.contains("- [ ] containing stable"));
        assert!(rendered.contains("Merge neither publishes a release"));
        assert!(!rendered.contains("Closes"));
    }
}

#[test]
fn manual() {
    let mut held = report(Target::Release);
    held.evaluation.verification = Some(Verification::Manual);
    let rendered = held.render();
    assert!(rendered.contains("verification: `manual`"));
    assert!(rendered.contains("Current verdict: `unknown`"));
    assert!(!rendered.contains("machine"));
}

#[test]
fn satisfied() {
    let mut held = report(Target::Source);
    held.evaluation.verdict = Verdict::Satisfied;
    held.evaluation.checks[0].fact.verdict = Verdict::Satisfied;
    assert!(
        held.render()
            .contains("not a prediction of merge or publication")
    );
}

#[test]
fn mixed() {
    let mut held = report(Target::Release);
    held.evaluation.checks.push(Check {
        name: "conditions".into(),
        fact: Fact {
            verdict: Verdict::Unmet,
            reason: "one native child remains open".into(),
        },
    });
    let rendered = held.render();
    assert!(rendered.contains("release: Unknown"));
    assert!(rendered.contains("conditions: Unmet"));
}

#[test]
fn retained() {
    let held = report(Target::Release);
    let encoded = serde_json::to_vec(&held).unwrap();
    let decoded: Report = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(held, decoded);
    assert_eq!(held.render(), decoded.render());
}

#[test]
fn unsupported() {
    let mut encoded = serde_json::to_value(report(Target::Release)).unwrap();
    encoded["evaluation"]["verification"] = "authenticated".into();
    assert!(serde_json::from_value::<Report>(encoded).is_err());
    let mut encoded = serde_json::to_value(report(Target::Release)).unwrap();
    encoded["evaluation"]["extra"] = true.into();
    assert!(serde_json::from_value::<Report>(encoded).is_err());
}

#[test]
fn unadopted() {
    let mut held = report(Target::Source);
    held.evaluation.target = None;
    held.evaluation.checks[0].name = "closure".into();
    let rendered = held.render();
    assert!(rendered.contains("Target: `not adopted`"));
    assert!(!rendered.contains("- closure:"));
}
