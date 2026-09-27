use super::{SCHEMA, SOURCE, TARGET};
use crate::estate::model::released;
use crate::{Attach, Edit, Fact, Import, Part, Patch, Role, Seat, Settle};
use keel::adapt::db::Sqlite;
use std::path::Path;
use std::process::Command;

#[tokio::test]
async fn inventory() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = source(temp.path()).await;
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
    assert_eq!(first.counts.retired_tasks, 0);
    assert_eq!(first.counts.facts, 1);
    assert_eq!(first.counts.phases, 1);
    assert_eq!(first.counts.phase_entries, 1);
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
    let seat = source(temp.path()).await;
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

async fn source(space: &Path) -> Seat {
    let seat = Seat::new(space);
    std::fs::create_dir_all(seat.database().parent().unwrap()).expect("estate directory");
    let wire = Sqlite::file(seat.database())
        .await
        .expect("source database");
    let mut held = keel::bootstrap(released(), wire).expect("released graph");
    let sudo = held.mint().await.expect("source sudo");
    let core = held.seal(&sudo).await.expect("released estate");
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

    let estate = seat.open().await.expect("released estate");
    estate.manage("local").await.expect("Domain");
    let task = estate.start("local", "alpha").await.expect("Task");
    let current = estate
        .change(&Patch {
            version: 1,
            task: task.identity(),
            revision: 0,
            edits: vec![Edit::Create {
                fact: Fact {
                    key: None,
                    role: Role::Goal,
                    rank: None,
                    title: None,
                    body: "Preserve the source".to_string(),
                    origin: None,
                },
            }],
        })
        .await
        .expect("current fact");
    let settled = estate
        .settle(&Settle {
            version: 1,
            task: task.identity(),
            revision: current.task.revision,
            phase: vec![crate::Entry {
                key: None,
                part: Part::Outcome,
                rank: None,
                title: None,
                body: "Source fixture exists".to_string(),
                origin: None,
            }],
            edits: Vec::new(),
        })
        .await
        .expect("Phase");

    let repository = space.join("repository");
    std::fs::create_dir(&repository).expect("repository directory");
    git(&repository, &["init", "-b", "main"]);
    git(&repository, &["config", "user.name", "Concord Test"]);
    git(
        &repository,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(repository.join("README.md"), "source\n").expect("repository payload");
    git(&repository, &["add", "README.md"]);
    git(&repository, &["commit", "-m", "source"]);
    estate
        .attach(&Attach {
            task: task.identity(),
            name: "worker".to_string(),
            source: repository,
            branch: Some("feature".to_string()),
            claims: vec!["README.md".to_string()],
            revision: settled.current.task.revision,
        })
        .await
        .expect("Member");

    let payload = space.join("payload.txt");
    std::fs::write(&payload, "evidence\n").expect("Artifact source");
    estate
        .import(&Import {
            task: task.identity(),
            name: "evidence".to_string(),
            source: payload,
        })
        .await
        .expect("Artifact");
    drop(estate);
    seat
}

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .expect("run Git");
    assert!(status.success(), "git {}", arguments.join(" "));
}
