use keel::adapt::{Error as Keel, Store};

#[test]
fn store() {
    let error = super::fault(Keel::Store(Store::Fault("disk I/O error".into())));
    assert_eq!(error.code(), "concord.estate.error");
    assert_eq!(error.message(), "store fault: disk I/O error");

    let error = super::upgrade(Keel::Store(Store::Down("database is locked".into())));
    assert_eq!(error.code(), "concord.estate.upgrade_required");
    assert_eq!(
        error.message(),
        "store unavailable: database is locked; this Concord release opens only its exact Issue estate"
    );
}

#[test]
fn other() {
    let error = super::fault(Keel::Missing("Anchor".into()));
    assert_eq!(error.message(), "missing resource: Anchor");

    let error = super::upgrade(Keel::Adapt("schema 3".into()));
    assert_eq!(
        error.message(),
        "adapt: schema 3; this Concord release opens only its exact Issue estate"
    );
}
