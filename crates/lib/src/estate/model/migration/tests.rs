use super::super::legacy;
use super::{RECEIPT, SCHEMA, SOURCE, TARGET};
use crate::Seat;
use keel::adapt::db::Sqlite;

#[tokio::test]
async fn cycle() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = source(temp.path()).await;
    let before = std::fs::read(seat.database()).expect("source database");

    let survey = seat.migration().survey().await.expect("source survey");
    assert_eq!(survey.schema, SCHEMA);
    assert_eq!(survey.source, SOURCE);
    assert_eq!(survey.target, TARGET);
    assert_eq!(
        std::fs::read(seat.database()).expect("surveyed database"),
        before
    );
    assert!(
        seat.open().await.is_err(),
        "legacy source must need migration"
    );
    assert!(seat.migration().prepare("wrong fingerprint").await.is_err());

    let plan = seat
        .migration()
        .prepare(&survey.fingerprint)
        .await
        .expect("prepared migration");
    assert!(plan.staged.path.is_file());
    assert_eq!(
        serde_json::from_value::<super::Plan>(serde_json::to_value(&plan).unwrap()).unwrap(),
        plan
    );
    assert_eq!(
        std::fs::read(seat.database()).expect("prepared source"),
        before
    );

    let receipt = seat
        .migration()
        .apply(&plan)
        .await
        .expect("applied migration");
    assert_eq!(receipt.schema, RECEIPT);
    assert!(receipt.backup.path.is_file());
    assert!(receipt.record.is_file());
    assert_eq!(std::fs::read(&receipt.backup.path).unwrap(), before);
    seat.open().await.expect("current estate");
    let retained: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt.record).unwrap()).unwrap();
    assert_eq!(retained["version"], 1);
    assert_eq!(retained["receipt"], serde_json::to_value(&receipt).unwrap());
    let mut forged = receipt.clone();
    forged.backup.path = seat.database();
    assert!(seat.migration().rollback(&forged).await.is_err());
    seat.open().await.expect("forged receipt changed nothing");

    let rollback = seat
        .migration()
        .rollback(&receipt)
        .await
        .expect("exact rollback");
    assert_eq!(rollback.source, SOURCE);
    assert_eq!(std::fs::read(seat.database()).unwrap(), before);
    assert!(
        seat.open().await.is_err(),
        "rollback restores legacy source"
    );
}

#[tokio::test]
async fn drift() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = source(temp.path()).await;
    let survey = seat.migration().survey().await.expect("source survey");
    let plan = seat
        .migration()
        .prepare(&survey.fingerprint)
        .await
        .expect("prepared migration");

    let sudo = std::fs::read_to_string(seat.sudo()).expect("sudo");
    let wire = Sqlite::file(seat.database())
        .await
        .expect("source database");
    let held = keel::bootstrap(legacy(), wire).expect("legacy graph");
    let core = held.seal(&sudo).await.expect("legacy estate");
    let space = core.live("Space").await.unwrap().remove(0).key();
    core.put(
        "Domain",
        &[
            ("name", "drift"),
            ("revision", "0"),
            ("space", &space.to_string()),
        ],
    )
    .await
    .expect("source drift");
    drop(core);

    let error = seat
        .migration()
        .apply(&plan)
        .await
        .expect_err("drift refusal");
    assert_eq!(error.code(), "concord.migration.drift");
    assert!(!plan.survey.backup.join("estate.sqlite3").exists());
}

async fn source(space: &std::path::Path) -> Seat {
    let seat = Seat::new(space);
    std::fs::create_dir_all(seat.database().parent().unwrap()).expect("estate directory");
    let wire = Sqlite::file(seat.database())
        .await
        .expect("source database");
    let mut held = keel::bootstrap(legacy(), wire).expect("legacy graph");
    let sudo = held.mint().await.expect("source sudo");
    let core = held.seal(&sudo).await.expect("legacy estate");
    core.put("Space", &[("name", "space"), ("revision", "0")])
        .await
        .expect("source Space");
    drop(core);
    crate::path::at(&seat.sudo())
        .file(&sudo)
        .expect("sudo file");
    crate::path::at(&seat.database())
        .mode(0o600)
        .expect("database mode");
    seat
}
