use concord_core::{Admission, Coordinate, Reconcile, Seat};

fn coordinate(raw: &str) -> Coordinate {
    Coordinate::parse(raw).expect("Issue coordinate")
}

#[tokio::test]
async fn attachment() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    let held = coordinate("PerishLab/concord#25");
    let anchor = estate
        .admit(&Admission {
            node: "I_issue".to_string(),
            coordinate: held.clone(),
        })
        .await
        .expect("attach Issue");
    assert_eq!(anchor.coordinate, held);
    assert_eq!(anchor.node, "I_issue");
    assert_eq!(anchor.revision, 0);
    assert_eq!(estate.issue(&held).await.expect("read anchor"), anchor);

    let duplicate = estate
        .admit(&Admission {
            node: "I_issue".to_string(),
            coordinate: coordinate("PerishLab/concord#26"),
        })
        .await
        .expect_err("node identity must be unique");
    assert_eq!(duplicate.code(), "concord.issue.node_reserved");
}

#[tokio::test]
async fn reconciliation() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    let before = coordinate("PerishLab/concord#25");
    estate
        .admit(&Admission {
            node: "I_issue".to_string(),
            coordinate: before.clone(),
        })
        .await
        .expect("attach Issue");
    let after = coordinate("PerishLab/plumb#40");
    let moved = estate
        .reconcile(&Reconcile {
            anchor: before.clone(),
            node: "I_issue".to_string(),
            coordinate: after.clone(),
            revision: 0,
        })
        .await
        .expect("reconcile coordinate");
    assert_eq!(moved.coordinate, after);
    assert_eq!(moved.revision, 1);
    assert_eq!(
        estate
            .issue(&before)
            .await
            .expect_err("old coordinate")
            .code(),
        "concord.issue.absent"
    );

    let stale = estate
        .reconcile(&Reconcile {
            anchor: after.clone(),
            node: "I_issue".to_string(),
            coordinate: coordinate("PerishLab/plumb#41"),
            revision: 0,
        })
        .await
        .expect_err("concurrent revision must refuse");
    assert_eq!(stale.code(), "concord.issue.stale");

    let mismatch = estate
        .reconcile(&Reconcile {
            anchor: after,
            node: "I_other".to_string(),
            coordinate: coordinate("PerishLab/plumb#41"),
            revision: 1,
        })
        .await
        .expect_err("node drift must refuse");
    assert_eq!(mismatch.code(), "concord.issue.node_mismatch");
}

#[test]
fn coordinates() {
    for raw in [
        "concord#25",
        "PerishLab/concord",
        "PerishLab/concord#0",
        "PerishLab/nested/concord#25",
    ] {
        assert!(Coordinate::parse(raw).is_err(), "{raw}");
    }
}
