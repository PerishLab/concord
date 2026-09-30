use super::{Fixture, fixture, released, request};
use concord_core::authority::Plumb;
use concord_core::issue_delivery;
use std::process::Command;

#[tokio::test(flavor = "current_thread")]
async fn moved() {
    let compiled = released();
    let Fixture {
        estate,
        issue,
        _temp,
    } = fixture(compiled.producer(), compiled.depot()).await;
    let member = _temp.path().join(".issues/I_delivery/worktree");
    let status = Command::new("git")
        .arg("-C")
        .arg(&member)
        .args(["commit", "--allow-empty", "--no-verify", "-m", "moved"])
        .status()
        .expect("git");
    assert!(status.success());
    let error = issue_delivery::prepare::<Plumb>(&estate, &request(&issue))
        .await
        .expect_err("moved HEAD");
    assert_eq!(error.code(), "concord.delivery.boundary");
    assert!(
        error.to_string().contains("Member HEAD moved from"),
        "{error}"
    );
}
