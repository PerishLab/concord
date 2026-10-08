use super::start::{admit, world};
use concord_core::{Register, Repository, Start};

#[tokio::test(flavor = "current_thread")]
async fn idle() {
    let world = world().await;
    let issue = admit(&world, 138, "I_retained").await;
    let artifact = world
        .temp
        .path()
        .join(".issues/I_retained/artifacts/history");
    std::fs::create_dir_all(&artifact).expect("historical Artifact");
    std::fs::write(artifact.join("evidence"), "retained").expect("retained payload");
    let held = world
        .estate
        .integrations()
        .await
        .expect("Integrations")
        .remove(0);
    assert_eq!(
        world.estate.retire(&held, false).await.expect("preflight"),
        held
    );
    assert_eq!(
        world.estate.integrations().await.expect("unchanged"),
        vec![held.clone()]
    );
    world.estate.retire(&held, true).await.expect("retire idle");
    assert!(world.source.join(".git").is_dir());
    assert_eq!(
        std::fs::read_to_string(artifact.join("evidence")).expect("retained Artifact"),
        "retained"
    );
    assert!(
        world
            .estate
            .integrations()
            .await
            .expect("retired")
            .is_empty()
    );
    assert_eq!(
        world
            .estate
            .issue(&issue)
            .await
            .expect("retained Anchor")
            .revision,
        0
    );
    assert!(world.estate.settled().await.expect("audit").agrees());
    assert_eq!(
        world
            .estate
            .retire(&held, true)
            .await
            .expect_err("no replay")
            .code(),
        "concord.integration.absent"
    );
    let next = world
        .estate
        .register(&Register {
            node: held.node.clone(),
            repository: held.repository.clone(),
            path: world.source.clone(),
        })
        .await
        .expect("reregister retained payload");
    assert_ne!(next.key, held.key);
    assert_eq!(
        world
            .estate
            .retire(&held, true)
            .await
            .expect_err("stale key")
            .code(),
        "concord.integration.changed"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn missing() {
    let world = world().await;
    let held = world
        .estate
        .integrations()
        .await
        .expect("Integrations")
        .remove(0);
    let retained = world.temp.path().join("removed");
    std::fs::rename(&world.source, &retained).expect("simulate domain removal");
    let broken = world.estate.settled().await.expect("broken audit");
    assert!(!broken.agrees());
    assert!(
        broken.faults[0]
            .message
            .contains("concord integration retire PerishLab/concord --node R_concord --key")
    );
    world
        .estate
        .retire(&held, false)
        .await
        .expect("missing preflight");
    world
        .estate
        .retire(&held, true)
        .await
        .expect("missing retirement");
    assert!(
        world
            .estate
            .settled()
            .await
            .expect("recovered audit")
            .agrees()
    );
    assert!(retained.join(".git").is_dir());
    assert!(!world.source.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn dependent() {
    let world = world().await;
    let issue = admit(&world, 138, "I_member").await;
    world
        .estate
        .start(&Start {
            issue,
            node: "I_member".into(),
            stable: "R_concord".into(),
            kind: "Feature".into(),
            claims: vec!["crates".into()],
            revision: 0,
        })
        .await
        .expect("Member");
    let held = world
        .estate
        .integrations()
        .await
        .expect("Integrations")
        .remove(0);
    for apply in [false, true] {
        assert_eq!(
            world
                .estate
                .retire(&held, apply)
                .await
                .expect_err("dependent Member")
                .code(),
            "concord.integration.dependent"
        );
    }
    assert!(
        world
            .estate
            .settled()
            .await
            .expect("preserved audit")
            .agrees()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn identity() {
    let world = world().await;
    let held = world
        .estate
        .integrations()
        .await
        .expect("Integrations")
        .remove(0);
    let mut changed = held.clone();
    changed.repository = Repository::parse("PerishLab/foreign").expect("foreign");
    assert_eq!(
        world
            .estate
            .retire(&changed, true)
            .await
            .expect_err("coordinate")
            .code(),
        "concord.integration.changed"
    );
    changed = held.clone();
    changed.path.push_str("/foreign");
    assert_eq!(
        world
            .estate
            .retire(&changed, true)
            .await
            .expect_err("payload identity")
            .code(),
        "concord.integration.changed"
    );
    assert_eq!(
        world.estate.integrations().await.expect("preserved"),
        vec![held]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unrelated() {
    let world = world().await;
    let held = world
        .estate
        .integrations()
        .await
        .expect("Integrations")
        .remove(0);
    let issue = admit(&world, 138, "I_foreign").await;
    let anchor = world.estate.issue(&issue).await.expect("Anchor");
    let root = world
        .temp
        .path()
        .join(".issues")
        .join(anchor.node)
        .join("worktree");
    std::fs::create_dir_all(&root).expect("unknown Member payload");
    assert_eq!(
        world
            .estate
            .retire(&held, true)
            .await
            .expect_err("unrelated fault")
            .code(),
        "concord.audit.refused"
    );
    assert_eq!(
        world.estate.integrations().await.expect("preserved"),
        vec![held]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn foreign() {
    let world = world().await;
    let peer = super::start::world().await;
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&peer.source)
        .args([
            "remote",
            "set-url",
            "origin",
            "https://github.com/PerishLab/foreign.git",
        ])
        .status()
        .expect("foreign origin");
    assert!(status.success());
    world
        .estate
        .register(&Register {
            node: "R_foreign".into(),
            repository: Repository::parse("PerishLab/foreign").expect("foreign"),
            path: peer.source.clone(),
        })
        .await
        .expect("foreign Integration");
    let held = world
        .estate
        .integrations()
        .await
        .expect("Integrations")
        .into_iter()
        .find(|held| held.node == "R_concord")
        .expect("target");
    std::fs::rename(&peer.source, peer.temp.path().join("removed")).expect("missing peer");
    for apply in [false, true] {
        assert_eq!(
            world
                .estate
                .retire(&held, apply)
                .await
                .expect_err("foreign fault")
                .code(),
            "concord.audit.refused"
        );
    }
    assert_eq!(
        world.estate.integrations().await.expect("preserved").len(),
        2
    );
}
