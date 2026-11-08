use super::super::start::{admit, world};
use concord_core::{Integration, Seat, Start};
use std::path::Path;
use std::process::Command;

#[tokio::test(flavor = "current_thread")]
async fn relocation() {
    let world = world().await;
    let issue = admit(&world, 454, "I_history").await;
    let before = world.estate.issue(&issue).await.expect("Anchor");
    let space = world.estate.identity().to_string();
    let held = registration(&world.estate).await;
    let path = world.temp.path().join("relocated");
    assert!(world.estate.relocation(&held, &world.source).await.is_err());
    std::fs::rename(&world.source, &path).expect("operator moves idle checkout");
    let artifact = world
        .temp
        .path()
        .join("workspace/issues/I_history/artifacts/evidence");
    std::fs::create_dir_all(&artifact).expect("Artifact");
    std::fs::write(artifact.join("source"), "retained").expect("payload");
    let plan = world
        .estate
        .relocation(&held, &path)
        .await
        .expect("prepare");
    let next = world.estate.rebind(&plan, false).await.expect("preflight");
    assert_eq!(registration(&world.estate).await, held);
    assert_ne!(next.path, held.path);
    assert_eq!(next.key, held.key);
    world.estate.rebind(&plan, true).await.expect("apply");
    assert_eq!(registration(&world.estate).await, next);
    assert_eq!(world.estate.identity(), space);
    assert_eq!(
        world.estate.issue(&issue).await.expect("retained Anchor"),
        before
    );
    assert_eq!(
        std::fs::read_to_string(artifact.join("source")).unwrap(),
        "retained"
    );
    assert!(world.estate.settled().await.expect("audit").agrees());
    assert!(world.estate.rebind(&plan, true).await.is_err());
    assert_eq!(registration(&world.estate).await, next);
}

#[tokio::test(flavor = "current_thread")]
async fn changed() {
    let world = world().await;
    let held = registration(&world.estate).await;
    let path = world.temp.path().join("relocated");
    std::fs::rename(&world.source, &path).unwrap();
    let plan = world.estate.relocation(&held, &path).await.unwrap();
    let mut stale = plan.clone();
    stale.integration.common.push_str("/changed");
    assert!(world.estate.rebind(&stale, true).await.is_err());
    let mut foreign = plan.clone();
    foreign.space = "0".repeat(64);
    assert!(world.estate.rebind(&foreign, true).await.is_err());
    foreign = plan.clone();
    foreign.schema = "concord.integration-rebind/v0".into();
    assert!(world.estate.rebind(&foreign, true).await.is_err());
    let other = tempfile::tempdir().unwrap();
    let estate = Seat::new(other.path()).bootstrap().await.unwrap();
    assert!(estate.rebind(&plan, true).await.is_err());
    std::fs::create_dir(&world.source).unwrap();
    assert!(world.estate.rebind(&plan, true).await.is_err());
    std::fs::remove_dir(&world.source).unwrap();
    std::fs::write(path.join("untracked"), "preserved").unwrap();
    assert!(world.estate.rebind(&plan, true).await.is_err());
    std::fs::remove_file(path.join("untracked")).unwrap();
    git(
        &path,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/PerishLab/other.git",
        ],
    );
    assert!(world.estate.rebind(&plan, true).await.is_err());
    git(
        &path,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/PerishLab/concord.git",
        ],
    );
    git(&path, &["commit", "--allow-empty", "-m", "changed head"]);
    git(&path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    assert!(world.estate.rebind(&plan, true).await.is_err());
    assert_eq!(registration(&world.estate).await, held);
    assert!(path.join(".git").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn dependent() {
    let world = world().await;
    let issue = admit(&world, 454, "I_active").await;
    world
        .estate
        .start(&Start {
            issue,
            node: "I_active".into(),
            stable: "R_concord".into(),
            kind: "Feature".into(),
            claims: vec!["crates".into()],
            revision: 0,
        })
        .await
        .unwrap();
    let held = registration(&world.estate).await;
    let path = world.temp.path().join("relocated");
    assert_eq!(
        world
            .estate
            .relocation(&held, &path)
            .await
            .unwrap_err()
            .code(),
        "concord.integration.dependent"
    );
    assert!(world.source.join(".git").is_dir());
}

#[tokio::test(flavor = "current_thread")]
async fn auxiliary() {
    let world = world().await;
    let held = registration(&world.estate).await;
    let path = world.temp.path().join("relocated");
    std::fs::rename(&world.source, &path).unwrap();
    let auxiliary = world.temp.path().join("auxiliary");
    git(
        &path,
        &["worktree", "add", "--detach", auxiliary.to_str().unwrap()],
    );
    assert!(world.estate.relocation(&held, &path).await.is_err());
    assert_eq!(registration(&world.estate).await, held);
    assert!(auxiliary.join("README.md").exists());
}

async fn registration(estate: &concord_core::Estate) -> Integration {
    estate.integrations().await.unwrap().remove(0)
}

fn git(path: &Path, args: &[&str]) {
    let result = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unrelated() {
    let world = world().await;
    let issue = admit(&world, 455, "I_orphan").await;
    let held = registration(&world.estate).await;
    let path = world.temp.path().join("relocated");
    std::fs::rename(&world.source, &path).unwrap();
    let plan = world.estate.relocation(&held, &path).await.unwrap();
    let root = world.temp.path().join("workspace/issues/I_orphan/worktree");
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(
        world.estate.rebind(&plan, true).await.unwrap_err().code(),
        "concord.audit.refused"
    );
    assert_eq!(registration(&world.estate).await, held);
    assert!(root.is_dir());
    assert_eq!(world.estate.issue(&issue).await.unwrap().revision, 0);
}
