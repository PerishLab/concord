use super::legacy;
use concord_core::{Part, Proving, Role, Seat};

#[tokio::test(flavor = "current_thread")]
async fn migration() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let space = legacy::space(temp.path());
    let memory = temp.path().join("local/.tasks/first/.task");

    let seat = Seat::new(temp.path());
    let survey = seat.survey(&space).expect("survey legacy Space");
    assert_eq!(survey.domain, 1);
    assert_eq!(survey.repository, 1);
    assert_eq!(survey.task, 2);
    assert_eq!(survey.member, 1);
    assert_eq!(survey.claim, 1);
    assert_eq!(survey.boundary, 1);
    assert_eq!(survey.dependency, 1);
    assert_eq!(survey.current, 6);
    assert_eq!(survey.phase, 1);
    assert_eq!(survey.artifact, 1);
    assert!(
        survey
            .evidence
            .iter()
            .all(|evidence| !evidence.kind.starts_with("member-"))
    );

    let member = temp.path().join("local/.tasks/first/repo");
    std::fs::write(member.join("transient"), "unmigrated member payload\n")
        .expect("write Member payload");
    assert_eq!(
        seat.survey(&space).expect("resurvey changed Member"),
        survey
    );
    std::fs::remove_file(member.join("transient")).expect("remove Member payload");

    let artifact = memory.join("resources/proof/payload");
    std::fs::write(&artifact, "changed evidence\n").expect("change Artifact payload");
    assert_ne!(
        seat.survey(&space)
            .expect("resurvey changed Artifact")
            .fingerprint,
        survey.fingerprint
    );
    std::fs::write(&artifact, "evidence\n").expect("restore Artifact payload");
    assert_eq!(
        seat.survey(&space).expect("resurvey restored Artifact"),
        survey
    );

    let staged = seat.stage(&space).await.expect("stage estate");
    assert_eq!(staged.census, survey);
    assert!(staged.root.join("estate.sqlite3").is_file());
    assert!(seat.open().await.is_err());
    drop(staged);
    let staged = seat
        .resume(&space, &survey.fingerprint)
        .await
        .expect("resume staged estate in a later session");
    assert!(memory.join("MAIN.md").is_file());
    assert!(memory.join("resources/proof/payload").is_file());
    let graph = staged.estate().graph(false).await.expect("staged graph");
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 1);
    let current = staged
        .estate()
        .current("local/first")
        .await
        .expect("staged current");
    assert_eq!(current.facts.len(), 6);
    let constraint = current
        .facts
        .iter()
        .find(|fact| fact.role == Role::Constraint)
        .expect("Constraint");
    assert_eq!(constraint.body, "- Preserve evidence.\n- Refuse drift.");
    assert!(current.facts.iter().any(|fact| fact.role == Role::Addition
        && fact.title.as_deref() == Some("legacy-task")
        && fact.body == "\"task-extra\""));
    let phases = staged
        .estate()
        .phases("local/first")
        .await
        .expect("staged Phases");
    assert_eq!(phases[0].entries.len(), 3);
    let decision = phases[0]
        .entries
        .iter()
        .find(|entry| entry.part == Part::Decision)
        .expect("Phase Decision");
    assert_eq!(
        decision.body,
        "- Keep one transaction.\n- Preserve bullets whole."
    );
    let member = &staged.estate().worktrees().await.expect("staged Members")[0];
    assert_eq!(member.source, "../repo");
    assert!(member.proof.is_some());

    let error = match staged.activate(&space, "wrong").await {
        Ok(_) => panic!("activation requires the exact Census fingerprint"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.migration.expect");
    assert!(!seat.database().exists());
    let staged = seat
        .resume(&space, &survey.fingerprint)
        .await
        .expect("resume after refused activation");
    let fingerprint = staged.census.fingerprint.clone();
    let active = staged
        .activate(&space, &fingerprint)
        .await
        .expect("activate estate");
    assert!(seat.database().is_file());
    assert!(active.root.join("MANIFEST.json").is_file());
    assert!(active.root.join("domains/local/tasks.toml").is_file());
    assert!(active.root.join("tasks/local/first/MAIN.md").is_file());
    assert!(
        active
            .root
            .join("tasks/local/first/phases/PHASE-00.md")
            .is_file()
    );
    let marker = std::fs::read_to_string(temp.path().join("local/.tasks/tasks.toml"))
        .expect("read refusal marker");
    assert!(marker.starts_with("version = 4\n"));
    assert!(
        !temp
            .path()
            .join("local/.tasks/first/.task/MAIN.md")
            .exists()
    );
    assert!(
        !temp
            .path()
            .join("local/.tasks/first/.task/resources")
            .exists()
    );
    assert!(
        temp.path()
            .join("local/.tasks/first/.task/artifacts/proof/payload")
            .is_file()
    );
    assert!(!temp.path().join("local/.tasks/second").exists());
    assert_eq!(
        active.estate().graph(false).await.expect("active graph"),
        graph
    );
    let agreement = active
        .estate()
        .inspect(None, None)
        .await
        .expect("inspect active estate");
    assert!(agreement.faults.is_empty());
    assert!(
        agreement
            .observations
            .iter()
            .any(|finding| finding.code == "boundary.stale")
    );
    let renewed = active
        .estate()
        .prove(&Proving {
            task: "local/first".to_string(),
            member: "repo".to_string(),
            revision: 0,
        })
        .await
        .expect("relative Member source remains operational");
    assert!(renewed.proof.is_some());
    assert_eq!(
        renewed.proof.as_ref().expect("renewed Boundary").plumb,
        concord_core::PLUMB
    );
}

#[tokio::test(flavor = "current_thread")]
async fn cycle() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let space = legacy::cycle(temp.path());
    let seat = Seat::new(temp.path());
    let error = match seat.stage(&space).await {
        Ok(_) => panic!("legacy todo cycle must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.migration.todo_cycle");
    assert!(
        !temp
            .path()
            .join(".concord/migration/v0.10.0/stage")
            .exists()
    );
    assert!(!seat.database().exists());
}
