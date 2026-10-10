use super::{fixture, released};
use concord_core::{IssueRelease, Recovery};
use std::process::Command;

#[tokio::test(flavor = "current_thread")]
async fn recovery() {
    let authority = released();
    let fixture = fixture(authority.producer(), authority.depot()).await;
    let member = fixture
        .estate
        .issue_member(&fixture.issue)
        .await
        .expect("Member");
    let source = fixture._temp.path().join("source");
    let path = fixture._temp.path().join(".issues/I_delivery/worktree");
    let before = fixture
        .estate
        .issue_member_status(&fixture.issue)
        .await
        .expect("status")
        .references;
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["merge", "--ff-only", &member.branch])
            .status()
            .expect("merge")
            .success()
    );
    let remote = fixture._temp.path().join("remote.git");
    for args in [
        vec![
            "push",
            remote.to_str().expect("remote"),
            "HEAD:refs/heads/main",
        ],
        vec![
            "fetch",
            remote.to_str().expect("remote"),
            "refs/heads/main:refs/remotes/origin/main",
        ],
    ] {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&source)
                .args(args)
                .status()
                .expect("sync origin")
                .success()
        );
    }
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&source)
            .arg("worktree")
            .arg("remove")
            .arg(&path)
            .status()
            .expect("remove")
            .success()
    );
    let request = Recovery {
        issue: fixture.issue.clone(),
        revision: 2,
        head: member.proof.as_ref().expect("proof").head.clone(),
    };
    fixture
        .estate
        .recover(&request)
        .await
        .expect("restore proven landed source");
    fixture.estate.recover(&request).await.expect("repeat");
    assert_eq!(
        fixture
            .estate
            .issue_member(&fixture.issue)
            .await
            .expect("Member"),
        member
    );
    assert_eq!(
        fixture
            .estate
            .issue_member_status(&fixture.issue)
            .await
            .expect("restored status")
            .references,
        before
    );
    assert_eq!(
        fixture
            .estate
            .release_issue(&IssueRelease {
                issue: fixture.issue,
                revision: 2
            })
            .await
            .expect("normal landed release"),
        3
    );
    assert!(fixture.estate.inspect().await.expect("audit").agrees());
}
