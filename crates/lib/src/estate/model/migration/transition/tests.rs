use super::{Observation, SCHEMA, SOURCE, TARGET};
use crate::{Coordinate, Seat};

#[tokio::test]
async fn inventory() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = super::fixture::source(temp.path()).await;
    let database = std::fs::read(seat.database()).expect("source database");
    let sudo = std::fs::read(seat.sudo()).expect("source sudo");
    let payload = temp.path().join("payload.txt");
    let original = std::fs::read(&payload).expect("source Artifact payload");

    let first = seat
        .transition()
        .inventory()
        .await
        .expect("first inventory");
    let second = seat
        .transition()
        .inventory()
        .await
        .expect("second inventory");
    assert_eq!(first, second);
    assert_eq!(first.schema, SCHEMA);
    assert_eq!(first.source, SOURCE);
    assert_eq!(first.target, TARGET);
    assert_eq!(first.counts.domains, 1);
    assert_eq!(first.counts.active_tasks, 1);
    assert_eq!(first.counts.retired_tasks, 1);
    assert_eq!(first.counts.facts, 2);
    assert_eq!(first.counts.phases, 2);
    assert_eq!(first.counts.phase_entries, 2);
    assert_eq!(first.counts.members, 1);
    assert_eq!(first.counts.claims, 1);
    assert_eq!(first.counts.boundaries, 0);
    assert_eq!(first.counts.artifacts, 1);
    assert_eq!(first.counts.filesystem_seats, 2);
    assert_eq!(first.tasks[0].revision, 3);
    assert_eq!(first.members[0].branch, "feature");
    assert_eq!(first.artifacts[0].entries.len(), 1);
    assert_eq!(
        serde_json::to_vec(&first).expect("first canonical JSON"),
        serde_json::to_vec(&second).expect("second canonical JSON")
    );
    assert_eq!(std::fs::read(seat.database()).unwrap(), database);
    assert_eq!(std::fs::read(seat.sudo()).unwrap(), sudo);
    assert_eq!(std::fs::read(&payload).unwrap(), original);

    let retained = first.artifacts[0].path.join("payload.txt");
    std::fs::write(&retained, "changed\n").expect("drift Artifact payload");
    let changed = seat
        .transition()
        .inventory()
        .await
        .expect("changed inventory");
    assert_ne!(changed.fingerprint, first.fingerprint);
    assert_ne!(changed.artifacts[0].digest, first.artifacts[0].digest);
}

#[tokio::test]
async fn preflight() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = super::fixture::source(temp.path()).await;
    let database = std::fs::read(seat.database()).expect("source database");
    let sudo = std::fs::read(seat.sudo()).expect("source sudo");
    let payload = std::fs::read(temp.path().join("payload.txt")).expect("Artifact payload");
    let inventory = seat.transition().inventory().await.expect("inventory");
    let plan = super::fixture::plan(temp.path(), &inventory);
    let observation = Observation {
        node: "I_node".to_string(),
        coordinate: Coordinate {
            owner: "PerishLab".to_string(),
            repository: "concord".to_string(),
            number: 37,
        },
    };

    let first = seat
        .transition()
        .preflight(&plan, std::slice::from_ref(&observation))
        .await
        .expect("preflight");
    let second = seat
        .transition()
        .preflight(&plan, std::slice::from_ref(&observation))
        .await
        .expect("repeat preflight");
    assert_eq!(first.inventory, second.inventory);
    assert_eq!(first.archive, second.archive);
    assert_eq!(first.tasks, second.tasks);
    assert_eq!(first.issues, second.issues);
    assert_eq!(first.members, second.members);
    assert_eq!(first.artifacts, second.artifacts);
    assert_eq!(first.capacity.required, second.capacity.required);
    assert_eq!(first.archive.tasks.len(), 1);
    assert_eq!(first.members.len(), 1);
    assert_eq!(first.artifacts.len(), 1);
    assert_eq!(std::fs::read(seat.database()).unwrap(), database);
    assert_eq!(std::fs::read(seat.sudo()).unwrap(), sudo);
    assert_eq!(
        std::fs::read(temp.path().join("payload.txt")).unwrap(),
        payload
    );

    let mut drift = plan.clone();
    drift.inventory = "changed".to_string();
    let error = seat
        .transition()
        .preflight(&drift, std::slice::from_ref(&observation))
        .await
        .expect_err("inventory drift");
    assert_eq!(error.code(), "concord.transition.inventory_drift");

    let mut provider = observation.clone();
    provider.node = "I_other".to_string();
    let error = seat
        .transition()
        .preflight(&plan, &[provider])
        .await
        .expect_err("provider drift");
    assert_eq!(error.code(), "concord.transition.provider_node_drift");

    let mut missing = plan.clone();
    missing.tasks.pop();
    let error = seat
        .transition()
        .preflight(&missing, std::slice::from_ref(&observation))
        .await
        .expect_err("missing disposition");
    assert_eq!(error.code(), "concord.transition.disposition_missing");

    std::fs::create_dir_all(&plan.members[0].path).expect("occupy target");
    let error = seat
        .transition()
        .preflight(&plan, &[observation])
        .await
        .expect_err("occupied target");
    assert_eq!(error.code(), "concord.transition.target_occupied");
}

#[tokio::test]
async fn refusal() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    drop(seat.bootstrap().await.expect("current estate"));
    let error = seat
        .transition()
        .inventory()
        .await
        .expect_err("current model must refuse");
    assert_eq!(error.code(), "concord.transition.source");
}

#[cfg(unix)]
#[tokio::test]
async fn symlink() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = super::fixture::source(temp.path()).await;
    let inventory = seat.transition().inventory().await.expect("inventory");
    symlink(
        temp.path().join("payload.txt"),
        inventory.artifacts[0].path.join("link"),
    )
    .expect("Artifact symlink");
    let error = seat
        .transition()
        .inventory()
        .await
        .expect_err("symlink must refuse");
    assert_eq!(error.code(), "concord.transition.artifact");
}
