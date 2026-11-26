use super::super::super::execution::unix::{git, text};
use super::super::unix::{guard, prepare, refused, success};
use super::Seat;
use serde_json::Value;
use std::path::Path;

#[test]
fn drift() {
    let fixture = tempfile::tempdir().expect("fixture");
    let seat = Seat::open(fixture.path());
    seat.change();
    let remote = seat.space.join("remote.git");
    let local = text(&seat.source, &["rev-parse", "refs/remotes/origin/main"]);
    let upstream = advance(seat.space, &remote);
    git(&remote, &["branch", "auto/88", "main"]);
    let output = prepare(seat.space, &seat.server, &seat.revision(), false)
        .output()
        .expect("prepare");
    refused(&output, "concord.delivery.stale");
    let error: Value = serde_json::from_slice(&output.stderr).expect("refusal JSON");
    assert_eq!(error["error"]["details"]["base"], "main");
    assert_eq!(error["error"]["details"]["local"], local);
    assert_eq!(error["error"]["details"]["remote"], upstream);
    assert_eq!(
        error["error"]["details"]["auto"],
        serde_json::json!(["auto/88"])
    );
    let message = error["error"]["message"].as_str().expect("message");
    assert!(message.contains(&upstream), "{message}");
    assert!(message.contains(&local), "{message}");
    assert!(message.contains("auto/88"), "{message}");

    git(&seat.source, &["fetch", "origin"]);
    let member = seat.space.join("workspace/issues/I_delivery/worktree");
    let revision = seat.revision();
    let proved = success(
        seat.space,
        &[
            "member",
            "prove",
            "PerishLab/probe#1",
            "--revision",
            &revision,
        ],
    );
    let advisory = proved["advisory"].as_str().expect("stale advisory");
    assert!(advisory.contains(&upstream), "{advisory}");
    assert!(advisory.contains("origin/main"), "{advisory}");

    git(&seat.source, &["merge", "--ff-only", "origin/main"]);
    git(&member, &["merge", "--no-edit", "origin/main"]);
    guard(&member, "PerishLab/probe");
    let revision = seat.revision();
    let clean = success(
        seat.space,
        &[
            "member",
            "prove",
            "PerishLab/probe#1",
            "--revision",
            &revision,
        ],
    );
    assert!(clean.get("advisory").is_none(), "{clean}");
    let prepared = prepare(seat.space, &seat.server, &seat.revision(), false)
        .output()
        .expect("prepare synced");
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
}

fn advance(space: &Path, remote: &Path) -> String {
    let peer = space.join("peer");
    git(
        space,
        &[
            "clone",
            "-b",
            "main",
            remote.to_str().expect("remote"),
            peer.to_str().expect("peer"),
        ],
    );
    git(&peer, &["config", "user.name", "Auto"]);
    git(&peer, &["config", "user.email", "auto@example.invalid"]);
    std::fs::write(peer.join("Cargo.lock"), "version = 4\n").expect("lock");
    git(&peer, &["add", "Cargo.lock"]);
    git(
        &peer,
        &["commit", "-m", "Follow the latest first-party packages"],
    );
    git(&peer, &["push", "origin", "main"]);
    text(&peer, &["rev-parse", "HEAD"])
}
