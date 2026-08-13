mod artifact;
mod brief;
mod changes;
mod coordinate;
mod work;

use concord_core::{Cut, Edge, Finish, Flow, Graph, Life, Link, Node, Origin, Seat, Tune, Weight};
use keel::adapt::db::Sqlite;
use keel::wire::Wire;

#[tokio::test(flavor = "current_thread")]
async fn genesis() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    drop(seat.bootstrap().await.expect("bootstrap estate"));
    drop(seat.open().await.expect("exact replay"));
    assert!(seat.database().is_file());
    assert!(seat.sudo().is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let database = std::fs::metadata(seat.database())
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777;
        let sudo = std::fs::metadata(seat.sudo())
            .expect("sudo metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!((database, sudo), (0o600, 0o600));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn drift() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    drop(seat.bootstrap().await.expect("bootstrap estate"));

    let mut wire = Sqlite::file(seat.database()).await.expect("open database");
    wire.script("CREATE TABLE drift (id INTEGER PRIMARY KEY NOT NULL)")
        .await
        .expect("inject physical drift");
    drop(wire);

    let error = match seat.open().await {
        Ok(_) => panic!("drift must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.estate.upgrade_required");
    assert!(error.to_string().contains("estate drift"));
}

#[tokio::test(flavor = "current_thread")]
async fn absence() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    let error = match seat.open().await {
        Ok(_) => panic!("absent estate must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.estate.absent");
    assert!(!temp.path().join(".concord").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn closure() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    let alpha = estate.start("local", "alpha").await.expect("start alpha");
    let beta = estate.start("local", "beta").await.expect("start beta");
    let gamma = estate.start("local", "gamma").await.expect("start gamma");
    assert!(!temp.path().join("local/.tasks/alpha").exists());

    let first = Link {
        source: alpha.identity(),
        target: beta.identity(),
        weight: Weight::Context,
        origin: Origin::Declared,
        revision: 0,
    };
    assert_eq!(estate.depend(&first).await.expect("first edge"), 1);
    let second = Link {
        source: beta.identity(),
        target: gamma.identity(),
        weight: Weight::Required,
        origin: Origin::Declared,
        revision: 1,
    };
    assert_eq!(estate.depend(&second).await.expect("second edge"), 2);

    let reached = estate
        .reach(&alpha.identity())
        .await
        .expect("closure reach");
    assert_eq!(
        reached
            .iter()
            .map(|node| node.identity())
            .collect::<Vec<_>>(),
        vec![beta.identity(), gamma.identity()]
    );
    let graph = estate.graph(false).await.expect("read graph");
    assert_eq!(graph.revision, 2);
    assert_eq!(
        graph.path(alpha.key, gamma.key, Weight::Unknown),
        Some(vec![alpha.key, beta.key, gamma.key])
    );
    assert!(graph.path(alpha.key, gamma.key, Weight::Required).is_none());

    let cycle = Link {
        source: gamma.identity(),
        target: alpha.identity(),
        weight: Weight::Unknown,
        origin: Origin::Declared,
        revision: 2,
    };
    let error = match estate.depend(&cycle).await {
        Ok(_) => panic!("cycle must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.dependency.cycle");
    assert_eq!(
        estate.graph(false).await.expect("unchanged graph").revision,
        2
    );

    let reflexive = Link {
        source: alpha.identity(),
        target: alpha.identity(),
        weight: Weight::Unknown,
        origin: Origin::Declared,
        revision: 2,
    };
    let error = match estate.depend(&reflexive).await {
        Ok(_) => panic!("self edge must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.dependency.self");

    let tune = Tune {
        source: alpha.identity(),
        target: beta.identity(),
        weight: Weight::Required,
        revision: 2,
    };
    assert_eq!(estate.weigh(&tune).await.expect("raise weight"), 3);
    let cut = Cut {
        source: beta.identity(),
        target: gamma.identity(),
        reason: "superseded".to_string(),
        revision: 3,
    };
    assert_eq!(estate.detach(&cut).await.expect("retire edge"), 4);
    assert_eq!(
        estate
            .reach(&alpha.identity())
            .await
            .expect("reduced closure")
            .iter()
            .map(|node| node.identity())
            .collect::<Vec<_>>(),
        vec![beta.identity()]
    );

    let finish = Finish {
        task: beta.identity(),
        revision: 0,
        graph: 4,
        reason: "completed".to_string(),
    };
    let retired = estate.finish(&finish).await.expect("retire Task");
    assert_eq!(retired.life, Life::Retired);
    assert_eq!(estate.graph(false).await.expect("active graph").revision, 5);
    assert!(
        estate
            .reach(&alpha.identity())
            .await
            .expect("empty closure")
            .is_empty()
    );
    let error = match estate.start("local", "beta").await {
        Ok(_) => panic!("retired name stays reserved"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.task.reserved");
}

#[test]
fn analysis() {
    let node = |key, name: &str| Node {
        key,
        domain: "local".to_string(),
        name: name.to_string(),
        life: Life::Active,
        revision: 0,
    };
    let edge = |key, source, target| Edge {
        key,
        source,
        target,
        weight: Weight::Unknown,
        origin: Origin::Declared,
    };
    let graph = Graph {
        revision: 1,
        nodes: vec![node(1, "alpha"), node(2, "beta"), node(3, "gamma")],
        edges: vec![edge(1, 1, 2), edge(2, 2, 1), edge(3, 2, 3)],
    };
    assert_eq!(graph.neighbors(3, Flow::In, 2, Weight::Unknown), vec![1, 2]);
    assert_eq!(graph.degree(2, Weight::Unknown).out, 2);
    assert_eq!(graph.scc(), vec![vec![1, 2], vec![3]]);
    assert_eq!(graph.cycles(), vec![vec![1, 2, 1]]);
}
