use super::{Native, git, prepare, refused, review, success, text};
use serde_json::json;

#[test]
fn early() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    std::fs::rename(
        seat.space.join("provider"),
        seat.space.join("offline-provider"),
    )
    .expect("remove provider access");
    refused(
        &seat.prepare("governance-manual"),
        "concord.delivery.review",
    );
    refused(
        &prepare(&seat, "plumb", &review(&seat)),
        "concord.delivery.review",
    );
}

#[test]
fn tool() {
    let temp = tempfile::tempdir().expect("fixture");
    let mut seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    seat.tools();
    let prepared = prepare(&seat, "governance-manual", &review(&seat));
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let actual = plumb::config::search(seat.space)
        .expect("search")
        .resolve("git")
        .expect("git");
    let quoted = actual.to_str().expect("path").replace('\'', "'\\''");
    plumb::testing::executable(
        &seat.space.join("tools/git"),
        format!("#!/bin/sh\nexec '{quoted}' \"$@\"\n\n"),
    );
    refused(
        &seat.land("governance-manual", &prepared.stdout),
        "concord.delivery.stale",
    );
    assert!(!seat.space.join("pull-state").exists());
}

#[test]
fn quoted() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    let mut input = review(&seat);
    input["reason"] =
        json!("Reviewed quoted text: Closes PerishLab/probe#999\n```\nRefs other/repo#2");
    let prepared = prepare(&seat, "governance-manual", &input);
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let landed = seat.land("governance-manual", &prepared.stdout);
    assert!(
        landed.status.success(),
        "{}",
        String::from_utf8_lossy(&landed.stderr)
    );
}

#[test]
fn trailer() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    git(
        &seat.member,
        &[
            "commit",
            "--amend",
            "-m",
            "manual delta\n\nPlumb-Guard-Proof: invalid",
        ],
    );
    success(
        seat.space,
        &["member", "prove", &seat.issue, "--revision", "2"],
    );
    refused(
        &prepare(&seat, "governance-manual", &review(&seat)),
        "concord.delivery.native",
    );
}

#[test]
fn moved() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    let prepared = prepare(&seat, "governance-manual", &review(&seat));
    assert!(prepared.status.success());
    std::fs::write(seat.source.join("README.md"), "new main\n").expect("base");
    git(&seat.source, &["add", "README.md"]);
    git(&seat.source, &["commit", "-m", "base moved"]);
    git(&seat.source, &["push", "origin", "main"]);
    let landed = seat.land("governance-manual", &prepared.stdout);
    assert!(!landed.status.success());
    assert!(!seat.space.join("pull-state").exists());
    assert_eq!(
        text(&seat.source, &["log", "-1", "--format=%s"]),
        "base moved"
    );
}
