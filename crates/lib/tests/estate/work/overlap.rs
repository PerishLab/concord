use concord_core::{Attach, Claiming, Seat};
use std::{path::Path, process::Command};

#[tokio::test(flavor = "current_thread")]
async fn overlap() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let source = temp.path().join("source");
    std::fs::create_dir(&source).expect("source directory");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);

    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    estate.start("local", "alpha").await.expect("start alpha");
    estate.start("local", "beta").await.expect("start beta");
    estate.start("local", "gamma").await.expect("start gamma");
    estate
        .attach(&Attach {
            task: "local/alpha".to_string(),
            name: "root".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["docs".to_string()],
            revision: 0,
        })
        .await
        .expect("attach root Member");
    let cross = estate
        .attach(&Attach {
            task: "local/beta".to_string(),
            name: "peer".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["docs/guide".to_string()],
            revision: 0,
        })
        .await
        .expect("attach cross-Task peer");
    assert_eq!(cross.observations.len(), 1);
    assert_eq!(cross.observations[0].peer, "local/alpha/root");
    let sibling = estate
        .attach(&Attach {
            task: "local/gamma".to_string(),
            name: "sibling".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["doc".to_string()],
            revision: 0,
        })
        .await
        .expect("attach component sibling");
    assert!(sibling.observations.is_empty());

    let same = estate
        .attach(&Attach {
            task: "local/beta".to_string(),
            name: "experiment".to_string(),
            source: source.clone(),
            branch: Some("beta-experiment".to_string()),
            claims: vec!["docs/guide/chapter".to_string()],
            revision: 1,
        })
        .await
        .expect("attach same-Task peer");
    assert_eq!(
        same.observations
            .iter()
            .map(|observation| observation.peer.as_str())
            .collect::<Vec<_>>(),
        ["local/alpha/root", "local/beta/peer"]
    );

    let expanded = estate
        .claim(&Claiming {
            task: "local/alpha".to_string(),
            member: "root".to_string(),
            claims: vec!["README.md".to_string()],
            revision: 1,
        })
        .await
        .expect("expand an overlapping Claim");
    assert_eq!(expanded.observations.len(), 2);
    let narrowed = estate
        .narrow(&concord_core::Narrowing {
            task: "local/alpha".to_string(),
            member: "root".to_string(),
            claims: vec!["docs/guide/chapter".to_string()],
            revision: 2,
        })
        .await
        .expect("narrow an overlapping Claim");
    assert_eq!(narrowed.observations.len(), 2);

    let agreement = estate.inspect(None, None).await.expect("audit estate");
    assert!(agreement.faults.is_empty());
    assert_eq!(
        agreement
            .observations
            .iter()
            .filter(|finding| finding.code == "claim.overlap")
            .count(),
        3
    );
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?}");
}
