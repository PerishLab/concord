use super::*;
use concord_core::{Import, Removal};

#[tokio::test(flavor = "current_thread")]
async fn repositories() {
    for fault in ["dirty", "missing", "extra"] {
        let world = World::new().await;
        world.foreign().await;
        let source = world.temp.path().join("plumb");
        let retained = world.temp.path().join("retained");
        match fault {
            "dirty" => std::fs::write(source.join("foreign"), "preserve").unwrap(),
            "missing" => std::fs::rename(&source, &retained).unwrap(),
            _ => git(
                &source,
                &["worktree", "add", "-b", "extra", retained.to_str().unwrap()],
            ),
        }
        let issue = world.start(41, "I_local", "R_concord").await;
        world
            .estate
            .claim(&Claiming {
                issue: issue.clone(),
                claims: vec!["docs".into()],
                revision: 1,
            })
            .await
            .unwrap();
        commit(&world.path("I_local"), "crates/probe.rs", "local");
        world
            .estate
            .prove(&Proving {
                issue: issue.clone(),
                revision: 2,
            })
            .await
            .unwrap();
        let input = world.temp.path().join("evidence");
        std::fs::write(&input, "retained evidence").unwrap();
        world
            .estate
            .import(&Import {
                issue,
                name: "evidence".into(),
                source: input,
                revision: 3,
            })
            .await
            .unwrap();
        let audit = world.estate.inspect().await.unwrap();
        assert!(!audit.agrees(), "full audit retains {fault}");
        if fault == "dirty" {
            assert_eq!(
                std::fs::read_to_string(source.join("foreign")).unwrap(),
                "preserve"
            );
        } else {
            assert!(retained.join("README.md").is_file());
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn auxiliary() {
    let world = World::new().await;
    let source = world.temp.path().join("concord");
    let extra = world.temp.path().join("auxiliary");
    git(
        &source,
        &[
            "worktree",
            "add",
            "-b",
            "auxiliary",
            extra.to_str().unwrap(),
        ],
    );
    let issue = world.start(41, "I_local", "R_concord").await;
    commit(&world.path("I_local"), "crates/probe.rs", "local");
    world
        .estate
        .prove(&Proving { issue, revision: 1 })
        .await
        .unwrap();
    assert!(extra.join("README.md").is_file());
    assert!(
        world
            .estate
            .inspect()
            .await
            .unwrap()
            .faults
            .iter()
            .any(|fault| fault.code == "integration.worktree.unknown")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn siblings() {
    let world = World::new().await;
    world.start(41, "I_broken", "R_concord").await;
    let path = world.path("I_broken");
    git(&path, &["switch", "-c", "moved"]);
    let issue = world.start(42, "I_local", "R_concord").await;
    commit(&world.path("I_local"), "crates/probe.rs", "local");
    world
        .estate
        .prove(&Proving { issue, revision: 1 })
        .await
        .unwrap();
    assert_eq!(
        Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["branch", "--show-current"])
            .output()
            .unwrap()
            .stdout,
        b"moved\n"
    );
    assert!(!world.estate.inspect().await.unwrap().agrees());
}

#[tokio::test(flavor = "current_thread")]
async fn target() {
    let world = World::new().await;
    let issue = world.start(41, "I_local", "R_concord").await;
    let source = world.temp.path().join("concord");
    std::fs::write(source.join("foreign"), "preserve").unwrap();
    let error = world
        .estate
        .prove(&Proving {
            issue: issue.clone(),
            revision: 1,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "concord.audit.refused");
    assert_eq!(world.estate.issue(&issue).await.unwrap().revision, 1);
    let input = world.temp.path().join("evidence");
    std::fs::write(&input, "evidence").unwrap();
    world
        .estate
        .import(&Import {
            issue: issue.clone(),
            name: "evidence".into(),
            source: input,
            revision: 1,
        })
        .await
        .unwrap();
    world
        .estate
        .remove(&Removal {
            issue,
            name: "evidence".into(),
            revision: 2,
        })
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(source.join("foreign")).unwrap(),
        "preserve"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn collision() {
    let world = World::new().await;
    let issue = world.start(41, "I_local", "R_concord").await;
    let source = world.temp.path().join("concord");
    let extra = world.temp.path().join("collision");
    git(
        &source,
        &[
            "worktree",
            "add",
            "--force",
            extra.to_str().unwrap(),
            "task/41",
        ],
    );
    let error = world
        .estate
        .prove(&Proving { issue, revision: 1 })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "concord.audit.refused");
    assert!(extra.join("README.md").is_file());
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn custody() {
    use std::os::unix::fs::PermissionsExt;
    let world = World::new().await;
    let root = world.temp.path().join("workspace");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    let issue = Coordinate::parse("PerishLab/concord#41").unwrap();
    let error = world
        .estate
        .admit(&Admission {
            node: "I_local".into(),
            coordinate: issue.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "concord.audit.refused");
    assert!(world.estate.issue(&issue).await.is_err());
}
