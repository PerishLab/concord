use concord_core::acceptance::{
    Check, Evaluation, Fact, Marker, Observation, Release, Target, Verdict, Verification, digest,
};

use crate::{closure, comment, declaration};

const BODY: &str = "## Acceptance\n- [x] Bounded source result verified\n";

struct World {
    target: Target,
    marker: Marker,
    body: String,
    freshness: Verdict,
    condition: Verdict,
    release: Verdict,
}

impl World {
    fn new(target: Target) -> Self {
        let first = comment("IC_first", &declaration(target));
        let mut marker = closure(&first, target);
        if let Marker::Closure {
            review, release, ..
        } = &mut marker
        {
            *review = digest(BODY);
            if target == Target::Release {
                *release = Some(Release {
                    marker: "v1.0.0".into(),
                    distribution: "https://example.org/distribution".into(),
                    inclusion: vec!["Reviewed source ancestry".into()],
                });
            }
        }
        Self {
            target,
            marker,
            body: BODY.into(),
            freshness: Verdict::Satisfied,
            condition: Verdict::Satisfied,
            release: Verdict::Unknown,
        }
    }

    fn read(&self) -> Evaluation {
        let comments = vec![
            comment("IC_first", &declaration(self.target)),
            comment("IC_closed", &self.marker),
        ];
        Evaluation::read(Observation {
            labels: &[self.target.label().into()],
            comments: &comments,
            body: &self.body,
            freshness: fact(self.freshness),
            conditions: vec![Check {
                name: "acceptance".into(),
                fact: fact(self.condition),
            }],
            release: fact(self.release),
        })
        .unwrap()
    }
}

#[test]
fn manual() {
    let evaluation = World::new(Target::Source).read();
    assert_eq!(evaluation.verdict, Verdict::Satisfied);
    assert_eq!(evaluation.verification, Some(Verification::Manual));
    let json = serde_json::to_value(evaluation).unwrap();
    assert_eq!(json["verification"], "manual");
    assert_eq!(json["checks"][2]["name"], "freshness");
}

#[test]
fn released() {
    let mut held = World::new(Target::Release);
    held.release = Verdict::Satisfied;
    let evaluation = held.read();
    assert_eq!(evaluation.verdict, Verdict::Satisfied);
    assert_eq!(evaluation.verification, Some(Verification::Manual));
}

#[test]
fn excluded() {
    let mut held = World::new(Target::Release);
    held.release = Verdict::Unmet;
    let evaluation = held.read();
    assert_eq!(evaluation.verdict, Verdict::Unmet);
    assert_eq!(evaluation.verification, Some(Verification::Manual));
    assert!(
        evaluation
            .checks
            .iter()
            .any(|check| check.name == "judgment" && check.fact.verdict == Verdict::Satisfied)
    );
    assert!(
        evaluation
            .checks
            .iter()
            .any(|check| check.name == "release" && check.fact.verdict == Verdict::Unmet)
    );
}

#[test]
fn edited() {
    let mut world = World::new(Target::Source);
    world.body.push_str("- [ ] Expanded acceptance\n");
    assert_eq!(world.read().verdict, Verdict::Unmet);
}

#[test]
fn restored() {
    let mut world = World::new(Target::Source);
    world.freshness = Verdict::Unmet;
    assert_eq!(world.read().verdict, Verdict::Unmet);
    world.freshness = Verdict::Unknown;
    assert_eq!(world.read().verdict, Verdict::Unknown);
}

#[test]
fn distribution() {
    let mut world = World::new(Target::Release);
    assert_eq!(world.read().verdict, Verdict::Unknown);
    world.release = Verdict::Unmet;
    assert_eq!(world.read().verdict, Verdict::Unmet);
    world.release = Verdict::Satisfied;
    assert_eq!(world.read().verdict, Verdict::Satisfied);
}

#[test]
fn obligations() {
    let mut world = World::new(Target::Source);
    world.condition = Verdict::Unmet;
    world.freshness = Verdict::Unknown;
    let evaluation = world.read();
    assert_eq!(evaluation.verdict, Verdict::Unmet);
    assert!(
        evaluation
            .checks
            .iter()
            .any(|check| check.fact.verdict == Verdict::Unknown)
    );
}

#[test]
fn absent() {
    let evaluation = Evaluation::read(Observation {
        labels: &[],
        comments: &[],
        body: BODY,
        freshness: fact(Verdict::Satisfied),
        conditions: Vec::new(),
        release: fact(Verdict::Satisfied),
    })
    .unwrap();
    assert_eq!(evaluation.target, None);
    assert_eq!(evaluation.verdict, Verdict::Unknown);
    assert_eq!(evaluation.verification, None);
}

#[test]
fn incomplete() {
    let world = World::new(Target::Source);
    let comments = vec![
        comment("IC_first", &declaration(Target::Source)),
        comment("IC_closed", &world.marker),
    ];
    let evaluation = Evaluation::read(Observation {
        labels: &[Target::Source.label().into()],
        comments: &comments,
        body: BODY,
        freshness: fact(Verdict::Satisfied),
        conditions: Vec::new(),
        release: fact(Verdict::Satisfied),
    })
    .unwrap();
    assert_eq!(evaluation.verdict, Verdict::Unknown);
}

#[test]
fn unresolved() {
    let evaluation = Evaluation::read(Observation {
        labels: &[],
        comments: &[],
        body: BODY,
        freshness: fact(Verdict::Unknown),
        conditions: vec![Check {
            name: "blockers".into(),
            fact: fact(Verdict::Unmet),
        }],
        release: fact(Verdict::Unknown),
    })
    .unwrap();
    assert_eq!(evaluation.verdict, Verdict::Unmet);
    assert!(
        evaluation
            .checks
            .iter()
            .any(|check| check.fact.verdict == Verdict::Unknown)
    );
}

fn fact(verdict: Verdict) -> Fact {
    Fact {
        verdict,
        reason: "Explicit adapter observation, not a citation-derived proof".into(),
    }
}
