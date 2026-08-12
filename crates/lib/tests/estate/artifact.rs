use concord_core::{Finish, Import, Life, Removal, Seat};

#[tokio::test(flavor = "current_thread")]
async fn artifact() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    estate.start("local", "held").await.expect("start Task");
    let task = temp.path().join("local/.tasks/held");
    assert!(!task.exists());

    let source = temp.path().join("evidence.txt");
    std::fs::write(&source, "proof\n").expect("write source");
    let import = Import {
        task: "local/held".to_string(),
        name: "evidence".to_string(),
        source: source.clone(),
    };
    let checked = estate.preflight(&import).await.expect("preflight Artifact");
    assert_eq!(checked.entries, 1);
    assert!(!task.exists());

    let artifact = estate.import(&import).await.expect("import Artifact");
    assert!(artifact.path.join("evidence.txt").is_file());
    assert_eq!(
        estate.artifacts("local/held").await.expect("Artifacts"),
        vec![artifact.clone()]
    );
    assert_eq!(estate.node("local/held").await.expect("Task").revision, 0);
    private(&artifact.path);
    private(&artifact.path.join("evidence.txt"));

    let finish = Finish {
        task: "local/held".to_string(),
        revision: 0,
        graph: 0,
        reason: "blocked".to_string(),
    };
    let error = estate
        .finish(&finish)
        .await
        .expect_err("retained Artifact must block finish");
    assert_eq!(error.code(), "concord.task.artifacts");

    estate
        .remove(&Removal {
            task: "local/held".to_string(),
            name: "evidence".to_string(),
        })
        .await
        .expect("remove Artifact");
    assert!(!task.exists());
    let retired = estate.finish(&finish).await.expect("retire Task");
    assert_eq!(retired.life, Life::Retired);

    estate.start("local", "legacy").await.expect("start Task");
    let legacy = temp.path().join("local/.tasks/legacy/.task");
    std::fs::create_dir_all(&legacy).expect("create legacy memory");
    std::fs::write(legacy.join("MAIN.md"), "legacy\n").expect("write legacy memory");
    let error = estate
        .finish(&Finish {
            task: "local/legacy".to_string(),
            revision: 0,
            graph: 1,
            reason: "blocked".to_string(),
        })
        .await
        .expect_err("legacy memory must be explicit foreign territory");
    assert_eq!(error.code(), "concord.audit.refused");
    let audit = estate
        .inspect(Some("local/legacy"), None)
        .await
        .expect("audit foreign territory");
    assert!(
        audit
            .faults
            .iter()
            .any(|fault| fault.code == "territory.memory")
    );
}

#[cfg(unix)]
fn private(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let held = std::fs::metadata(path)
        .expect("Artifact metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(held, if path.is_dir() { 0o700 } else { 0o600 });
}

#[cfg(not(unix))]
fn private(_: &std::path::Path) {}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn linked() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    estate.start("local", "linked").await.expect("start Task");
    let source = temp.path().join("source");
    std::fs::create_dir(&source).expect("create source");
    std::fs::write(source.join("held"), "proof\n").expect("write source");
    symlink(source.join("held"), source.join("alias")).expect("link source");
    let error = estate
        .preflight(&Import {
            task: "local/linked".to_string(),
            name: "evidence".to_string(),
            source,
        })
        .await
        .expect_err("linked Artifact source must refuse");
    assert!(error.to_string().contains("refuses symbolic link"));
    assert!(!temp.path().join("local/.tasks/linked").exists());
}
