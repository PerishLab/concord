use super::{Disposition, DispositionPlan, IssueDestination, ResourceDisposition, TaskDisposition};
use crate::estate::model::released;
use crate::{Attach, Coordinate, Edit, Fact, Finish, Import, Part, Patch, Role, Seat, Settle};
use keel::adapt::db::Sqlite;
use std::path::Path;
use std::process::Command;

pub async fn source(space: &Path) -> Seat {
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
                fact: fact("Preserve the source"),
            }],
        })
        .await
        .expect("current fact");
    let settled = estate
        .settle(&Settle {
            version: 1,
            task: task.identity(),
            revision: current.task.revision,
            phase: vec![entry("Source fixture exists")],
            edits: Vec::new(),
        })
        .await
        .expect("Phase");
    retired(&estate).await;
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

async fn retired(estate: &crate::Estate) {
    let task = estate
        .start("local", "retired")
        .await
        .expect("retired Task");
    let current = estate
        .change(&Patch {
            version: 1,
            task: task.identity(),
            revision: task.revision,
            edits: vec![Edit::Create {
                fact: fact("Preserve retired history"),
            }],
        })
        .await
        .expect("retired current fact");
    let settled = estate
        .settle(&Settle {
            version: 1,
            task: task.identity(),
            revision: current.task.revision,
            phase: vec![entry("Retired history exists")],
            edits: Vec::new(),
        })
        .await
        .expect("retired Phase");
    let graph = estate.graph(true).await.expect("source graph");
    estate
        .finish(&Finish {
            task: task.identity(),
            revision: settled.current.task.revision,
            graph: graph.revision,
            reason: "Archived by transition fixture".to_string(),
        })
        .await
        .expect("retire Task");
}

pub fn plan(space: &Path, inventory: &super::Inventory) -> DispositionPlan {
    let issue = IssueDestination {
        node: "I_node".to_string(),
        coordinate: Coordinate {
            owner: "PerishLab".to_string(),
            repository: "concord".to_string(),
            number: 37,
        },
    };
    let tasks = inventory
        .tasks
        .iter()
        .map(|task| TaskDisposition {
            task: task.identity.clone(),
            revision: task.revision,
            disposition: if task.life == crate::Life::Active {
                Disposition::Issue {
                    issue: issue.clone(),
                }
            } else {
                Disposition::ArchiveOnly
            },
        })
        .collect();
    DispositionPlan {
        schema: super::preflight::SCHEMA.to_string(),
        inventory: inventory.fingerprint.clone(),
        tasks,
        members: vec![resource(space, &issue, "members", "worker")],
        artifacts: vec![resource(space, &issue, "artifacts", "evidence")],
    }
}

fn resource(
    space: &Path,
    issue: &IssueDestination,
    family: &str,
    name: &str,
) -> ResourceDisposition {
    ResourceDisposition {
        task: "local/alpha".to_string(),
        name: name.to_string(),
        node: issue.node.clone(),
        path: space
            .join(".issues")
            .join(&issue.node)
            .join(family)
            .join(name),
    }
}

fn fact(body: &str) -> Fact {
    Fact {
        key: None,
        role: Role::Goal,
        rank: None,
        title: None,
        body: body.to_string(),
        origin: None,
    }
}

fn entry(body: &str) -> crate::Entry {
    crate::Entry {
        key: None,
        part: Part::Outcome,
        rank: None,
        title: None,
        body: body.to_string(),
        origin: None,
    }
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
