use super::super::super::execution::unix::git;
use super::super::provider::{consume, projection, tool};
use super::super::unix::{prepare, refused};
use super::Seat;
use serde_json::{Value, json};

#[test]
fn branches() {
    let fixture = tempfile::tempdir().expect("fixture");
    let seat = Seat::open(fixture.path());
    let space = seat.space;
    let provider = seat.provider.as_str();
    seat.change();
    let remote = space.join("remote.git");
    for branch in [
        "release/v1.2.3",
        "auto/55",
        "feature/7",
        "task/5",
        "bug/6",
        "task/8",
        "land/task/9",
        "stray",
    ] {
        git(&remote, &["branch", branch, "main"]);
    }
    git(&seat.source, &["branch", "auto/56"]);
    git(&seat.source, &["branch", "topic"]);
    let issues = json!({"data": {"repository": {
        "i5": issue("CLOSED", "Task"),
        "i6": issue("OPEN", "Task"),
        "i7": issue("OPEN", "Feature"),
        "i8": null,
    }}, "errors": [{"type": "NOT_FOUND", "path": ["repository", "i8"], "message": "missing"}]});
    std::fs::write(space.join("branch-issues"), issues.to_string()).expect("issues");
    let output = prepare(space, provider, &seat.revision(), false)
        .output()
        .expect("prepare");
    refused(&output, "concord.delivery.branches");
    let error: Value = serde_json::from_slice(&output.stderr).expect("refusal JSON");
    assert_eq!(error["error"]["details"]["repository"], "PerishLab/probe");
    assert_eq!(
        error["error"]["details"]["branches"],
        json!([
            {"branch": "bug/6", "place": "origin", "reason": "type"},
            {"branch": "land/task/9", "place": "origin", "reason": "delivery"},
            {"branch": "stray", "place": "origin", "reason": "name"},
            {"branch": "task/5", "place": "origin", "reason": "closed"},
            {"branch": "task/8", "place": "origin", "reason": "absent"},
            {"branch": "topic", "place": "local", "reason": "name"},
        ])
    );
    let message = error["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("origin task/5 (closed Issue)"),
        "{message}"
    );
    assert!(!message.contains("feature/7"), "{message}");

    std::fs::remove_file(space.join("branch-issues")).expect("provider failure");
    refused(
        &prepare(space, provider, &seat.revision(), false)
            .output()
            .expect("prepare"),
        "concord.delivery.provider",
    );

    for branch in ["task/5", "bug/6", "task/8", "land/task/9", "stray"] {
        git(&remote, &["branch", "-D", branch]);
    }
    git(&seat.source, &["branch", "-D", "topic"]);
    let open = json!({"data": {"repository": {
        "i7": issue("OPEN", "Feature"),
    }}});
    std::fs::write(space.join("branch-issues"), open.to_string()).expect("issues");
    let plan = prepare(space, provider, &seat.revision(), true)
        .output()
        .expect("prepare");
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let before = seat.revision();
    git(&seat.source, &["branch", "topic"]);
    let output = consume(space, provider, &plan.stdout);
    refused(&output, "concord.delivery.branches");
    assert!(!space.join("pull-state").exists());
    assert_eq!(seat.revision(), before);
    git(&seat.source, &["branch", "-D", "topic"]);
    let landed = consume(space, provider, &plan.stdout);
    assert!(
        landed.status.success(),
        "{}",
        String::from_utf8_lossy(&landed.stderr)
    );
}

fn issue(state: &str, kind: &str) -> Value {
    json!({"__typename": "Issue", "state": state, "issueType": {"name": kind}})
}

#[test]
fn sections() {
    let fixture = tempfile::tempdir().expect("fixture");
    let seat = Seat::open(fixture.path());
    for (kind, heading, section) in [
        ("Bug", "Expected outcome", "expected outcome"),
        ("Feature", "Outcome", "outcome"),
        ("Task", "Outcome", "outcome"),
    ] {
        for body in [
            "",
            &format!("## {heading}\n \n"),
            &format!("## {heading}\nFixed"),
        ] {
            let encoded = serde_json::to_string(body).unwrap();
            let script = projection(seat.space).replace("Task", kind).replace(
                "## Outcome\\nPipe exact plan",
                &encoded[1..encoded.len() - 1],
            );
            let provider = tool(seat.space, "outcome-provider", script);
            let structured = invoke(&seat, &provider, true);
            assert!(!structured.status.success());
            let error: Value = serde_json::from_slice(&structured.stderr).expect("error JSON");
            if body.ends_with("Fixed") {
                assert_ne!(
                    error["error"]["code"], "concord.delivery.outcome",
                    "{error}"
                );
                continue;
            }
            assert_eq!(
                error["error"]["code"], "concord.delivery.outcome",
                "{error}"
            );
            assert_eq!(error["error"]["details"]["kind"], kind);
            assert_eq!(error["error"]["details"]["section"], section);
            let message = error["error"]["message"].as_str().unwrap();
            assert!(message.contains(&format!("type {kind}")));
            assert!(message.contains(&format!("`## {heading}`")));
            let human = invoke(&seat, &provider, false);
            assert!(!human.status.success());
            assert!(String::from_utf8_lossy(&human.stderr).contains(message));
        }
    }
}

fn invoke(seat: &Seat<'_>, provider: &std::path::Path, json: bool) -> std::process::Output {
    let mut command = super::super::super::spawn::concord(seat.space);
    command.args(["--root", seat.space.to_str().unwrap()]);
    if json {
        command.arg("--json");
    }
    command
        .args([
            "issue",
            "delivery",
            "prepare",
            "PerishLab/probe#1",
            "--revision",
            &seat.revision(),
            "--github-command",
            provider.to_str().unwrap(),
        ])
        .output()
        .expect("prepare")
}
