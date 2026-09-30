use concord_core::{PLUMB, Proof};

fn proof() -> Proof {
    Proof {
        key: 1,
        schema: plumb::boundary::SCHEMA.to_owned(),
        plumb: PLUMB.to_owned(),
        base: "base".to_owned(),
        head: "head".to_owned(),
        claim: "claim".to_owned(),
    }
}

#[test]
fn current() {
    assert_eq!(proof().stale("head", "claim"), None);
    assert!(proof().current("head", "claim"));
}

#[test]
fn linked() {
    let proof = Proof {
        plumb: "0.0.1".to_owned(),
        ..proof()
    };
    let reason = proof.stale("head", "claim").unwrap();
    assert!(reason.contains("made with Plumb 0.0.1"), "{reason}");
    assert!(reason.contains(&format!("links Plumb {PLUMB}")), "{reason}");
    assert!(!proof.current("head", "claim"));
}

#[test]
fn schema() {
    let proof = Proof {
        schema: "retired".to_owned(),
        ..proof()
    };
    let reason = proof.stale("moved", "changed").unwrap();
    assert!(reason.contains("Boundary schema retired"), "{reason}");
}

#[test]
fn head() {
    let reason = proof().stale("moved", "claim").unwrap();
    assert!(reason.contains("HEAD moved from head to moved"), "{reason}");
}

#[test]
fn claim() {
    let reason = proof().stale("head", "changed").unwrap();
    assert!(reason.contains("Claim changed"), "{reason}");
}
