use concord_core::{Annotate, Import, Life, Link, Origin, Rehome, Rename, Retire, Seat, Weight};

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
    estate.manage("other").await.expect("manage other Domain");
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
    assert_eq!(repo.life, Life::Active);
    assert_eq!(repo.reason, None);
    let (legacy, revision) = estate
        .annotate(&Annotate {
            domain: "local".to_string(),
            name: "legacy".to_string(),
            note: Some("RETIRED".to_string()),
            revision: 1,
        })
        .await
        .expect("annotate deleted Repository without probing it");
    assert_eq!(revision, 2);
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
            revision: 2,
        })
        .await
        .expect("clear Repository note");
    assert_eq!(revision, 3);
    assert_eq!(repo.note, None);
    let unchanged = repo.clone();
    let error = estate
        .tombstone(&Retire {
            domain: "local".to_string(),
            name: "legacy".to_string(),
            reason: " ".to_string(),
            revision: 3,
        })
        .await
        .expect_err("blank retirement reason must refuse");
    assert_eq!(error.code(), "concord.repository.reason");
    let error = estate
        .tombstone(&Retire {
            domain: "local".to_string(),
            name: "legacy".to_string(),
            reason: "product was absorbed".to_string(),
            revision: 2,
        })
        .await
        .expect_err("stale Domain revision must refuse retirement");
    assert_eq!(error.code(), "concord.domain.stale");
    let error = estate
        .tombstone(&Retire {
            domain: "local".to_string(),
            name: "absent".to_string(),
            reason: "never existed".to_string(),
            revision: 3,
        })
        .await
        .expect_err("absent Repository must refuse retirement");
    assert_eq!(error.code(), "concord.repository.absent");
    let (retired, revision) = estate
        .tombstone(&Retire {
            domain: "local".to_string(),
            name: "legacy".to_string(),
            reason: "product was absorbed".to_string(),
            revision: 3,
        })
        .await
        .expect("retire Repository annotation");
    assert_eq!(revision, 4);
    assert_eq!(retired.key, legacy.key);
    assert_eq!(retired.life, Life::Retired);
    assert_eq!(retired.reason.as_deref(), Some("product was absorbed"));
    assert_eq!(
        estate
            .repositories(Some("local"), false)
            .await
            .expect("list Repositories"),
        vec![unchanged.clone()]
    );
    assert_eq!(
        estate
            .repositories(Some("local"), true)
            .await
            .expect("list retained Repositories"),
        vec![unchanged, retired]
    );
    assert!(
        estate
            .repositories(Some("other"), false)
            .await
            .expect("list another Domain")
            .is_empty()
    );
    let error = estate
        .tombstone(&Retire {
            domain: "local".to_string(),
            name: "legacy".to_string(),
            reason: "again".to_string(),
            revision: 4,
        })
        .await
        .expect_err("repeated retirement must refuse");
    assert_eq!(error.code(), "concord.repository.retired");
    let error = estate
        .annotate(&Annotate {
            domain: "local".to_string(),
            name: "legacy".to_string(),
            note: Some("resurrect".to_string()),
            revision: 4,
        })
        .await
        .expect_err("annotating a retired Repository must refuse");
    assert_eq!(error.code(), "concord.repository.retired");
    assert!(
        estate
            .inspect(None, None)
            .await
            .expect("audit estate")
            .agrees()
    );
}
