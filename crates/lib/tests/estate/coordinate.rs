use concord_core::{Annotate, Import, Link, Origin, Rehome, Rename, Seat, Weight};

#[tokio::test(flavor = "current_thread")]
async fn coordinate() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage local");
    estate.manage("other").await.expect("manage other");
    let alpha = estate.start("local", "alpha").await.expect("start alpha");
    let beta = estate.start("local", "beta").await.expect("start beta");
    estate
        .depend(&Link {
            source: alpha.identity(),
            target: beta.identity(),
            weight: Weight::Sequence,
            origin: Origin::Declared,
            revision: 0,
        })
        .await
        .expect("declare dependency");
    let source = temp.path().join("evidence.txt");
    std::fs::write(&source, "retained\n").expect("write Artifact source");
    estate
        .import(&Import {
            task: alpha.identity(),
            name: "evidence".to_string(),
            source,
        })
        .await
        .expect("import Artifact");

    let renamed = estate
        .rename(&Rename {
            task: alpha.identity(),
            name: "renamed".to_string(),
            revision: 0,
        })
        .await
        .expect("rename Task");
    assert_eq!(renamed.key, alpha.key);
    assert_eq!(renamed.identity(), "local/renamed");
    assert!(
        temp.path()
            .join("local/.tasks/renamed/.task/artifacts/evidence/evidence.txt")
            .is_file()
    );
    let error = estate
        .start("local", "alpha")
        .await
        .expect_err("old coordinate stays reserved");
    assert_eq!(error.code(), "concord.task.reserved");

    let moved = estate
        .rehome(&Rehome {
            task: renamed.identity(),
            domain: "other".to_string(),
            revision: 1,
        })
        .await
        .expect("rehome Task");
    assert_eq!(moved.key, alpha.key);
    assert_eq!(moved.identity(), "other/renamed");
    assert_eq!(moved.revision, 2);
    assert!(
        temp.path()
            .join("other/.tasks/renamed/.task/artifacts/evidence/evidence.txt")
            .is_file()
    );
    let graph = estate.graph(false).await.expect("read graph");
    assert_eq!(graph.edges[0].source, alpha.key);
    assert_eq!(graph.edges[0].target, beta.key);
    assert_eq!(graph.revision, 1);
    let error = estate
        .start("local", "renamed")
        .await
        .expect_err("source coordinate stays reserved after rehome");
    assert_eq!(error.code(), "concord.task.reserved");
}

#[tokio::test(flavor = "current_thread")]
async fn repository() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    let (repo, revision) = estate
        .annotate(&Annotate {
            domain: "local".to_string(),
            name: "concord".to_string(),
            note: Some("integration".to_string()),
            revision: 0,
        })
        .await
        .expect("annotate Repository");
    assert_eq!(revision, 1);
    assert_eq!(repo.note.as_deref(), Some("integration"));
    let error = estate
        .annotate(&Annotate {
            domain: "local".to_string(),
            name: "concord".to_string(),
            note: None,
            revision: 0,
        })
        .await
        .expect_err("stale Domain revision must refuse");
    assert_eq!(error.code(), "concord.domain.stale");
    let (repo, revision) = estate
        .annotate(&Annotate {
            domain: "local".to_string(),
            name: "concord".to_string(),
            note: None,
            revision: 1,
        })
        .await
        .expect("clear Repository note");
    assert_eq!(revision, 2);
    assert_eq!(repo.note, None);
    assert_eq!(
        estate
            .repositories(Some("local"))
            .await
            .expect("list Repositories"),
        vec![repo]
    );
}
