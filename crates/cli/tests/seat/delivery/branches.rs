use super::super::super::execution::unix::git;
use super::super::provider::consume;
use super::super::unix::{prepare, refused};
use super::Seat;
use serde_json::{Value, json};

#[test]
fn branches() {
    let fixture = tempfile::tempdir().expect("fixture");
    let seat = Seat::open(fixture.path());
    let space = seat.space;
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
        "i9": issue("OPEN", "Task"),
    }}, "errors": [{"type": "NOT_FOUND", "path": ["repository", "i8"], "message": "missing"}]});
    std::fs::write(space.join("branch-issues"), issues.to_string()).expect("issues");
    std::fs::write(space.join("branch-projections"), json!([]).to_string()).unwrap();
    let output = run(&seat, false);
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
    refused(&run(&seat, false), "concord.delivery.provider");

    for branch in ["task/5", "bug/6", "task/8", "land/task/9", "stray"] {
        git(&remote, &["branch", "-D", branch]);
    }
    git(&seat.source, &["branch", "-D", "topic"]);
    let open = json!({"data": {"repository": {"i7": issue("OPEN", "Feature")}}});
    std::fs::write(space.join("branch-issues"), open.to_string()).expect("issues");
    let plan = run(&seat, true);
    ok(&plan);
    let before = seat.revision();
    git(&seat.source, &["branch", "topic"]);
    let output = consume(space, Some(&seat.server), &plan.stdout);
    refused(&output, "concord.delivery.branches");
    assert!(!space.join("pull-state").exists());
    assert_eq!(seat.revision(), before);
    git(&seat.source, &["branch", "-D", "topic"]);
    ok(&consume(space, Some(&seat.server), &plan.stdout));
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
        for (body, valid) in [
            (String::new(), false),
            (format!("## {heading}\n \n"), false),
            (format!("### {heading}\n \n"), false),
            (format!("## {heading}\nFixed"), true),
            (
                format!("### Problem\nCtx\n### {heading}\nFixed\n### Acceptance\n- [x] Ok"),
                true,
            ),
        ] {
            std::fs::write(seat.space.join("issue-kind"), kind).expect("kind");
            std::fs::write(seat.space.join("issue-body"), &body).expect("body");
            let structured = invoke(&seat, true);
            assert!(!structured.status.success());
            let error: Value = serde_json::from_slice(&structured.stderr).expect("error JSON");
            if valid {
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
            assert!(message.contains(&format!("`### {heading}`")));
            let human = invoke(&seat, false);
            assert!(!human.status.success());
            assert!(String::from_utf8_lossy(&human.stderr).contains(message));
        }
    }
    seat.change();
    for (kind, heading) in [
        ("Task", "Outcome"),
        ("Feature", "Outcome"),
        ("Bug", "Expected outcome"),
    ] {
        let form = format!(
            "### Problem\nCtx\n### Evidence\nLog\n### {heading}\nPipe {kind} H3\n### Scope\nParser\n### Acceptance\n- [x] Done\n### Non-goals\nNone"
        );
        std::fs::write(seat.space.join("issue-kind"), kind).expect("kind");
        std::fs::write(seat.space.join("issue-body"), form).expect("body");
        let prepared = invoke(&seat, true);
        ok(&prepared);
        let plan: Value = serde_json::from_slice(&prepared.stdout).expect("plan JSON");
        let body = plan["plan"]["delivery"]["pull"]["body"].as_str().unwrap();
        assert!(body.contains(&format!("Pipe {kind} H3")), "{body}");
    }
}

fn invoke(seat: &Seat<'_>, json: bool) -> std::process::Output {
    let mut command = super::super::super::spawn::concord(seat.space);
    seat.server.attach(&mut command);
    command.args(["--home", seat.space.to_str().unwrap()]);
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
        ])
        .output()
        .expect("prepare")
}

fn pull(branch: &str, head: &str, number: u64) -> Value {
    let origin = json!({"id": 1, "node_id": "R_probe", "full_name": "PerishLab/probe"});
    json!({
        "id": 100 + number, "number": number, "node_id": format!("PR_{number}"),
        "html_url": format!("https://github.com/PerishLab/probe/pull/{number}"),
        "title": "Retained delivery", "body": "Refs PerishLab/probe#9\n\nRetained delivery.",
        "state": "open", "draft": false, "merged": false,
        "merge_commit_sha": null, "merged_at": null, "updated_at": "2026-09-29T00:00:03Z",
        "head": {"ref": branch, "sha": head, "repo": origin.clone()},
        "base": {"ref": "main", "sha": head, "repo": origin},
    })
}

fn mutate(base: &Value, field: &str) -> Value {
    let mut wrong = base.clone();
    match field {
        "sha" => wrong["head"]["sha"] = json!("0".repeat(40)),
        "body" => wrong["body"] = json!("mismatch"),
        "state" => wrong["state"] = json!("closed"),
        "branch" => wrong["head"]["ref"] = json!("mismatch"),
        "head" => wrong["head"]["repo"]["full_name"] = json!("PerishLab/other"),
        _ => wrong["base"]["repo"]["full_name"] = json!("PerishLab/other"),
    }
    wrong
}

#[test]
fn foreign() {
    let fixture = tempfile::tempdir().unwrap();
    let seat = Seat::open(fixture.path());
    seat.change();
    let remote = seat.space.join("remote.git");
    let branch = "land/feature/9";
    git(&remote, &["branch", branch, "main"]);
    let retained = head(&remote, branch);
    let issues = json!({"data":{"repository":{"i9":issue("OPEN", "Feature")}}});
    std::fs::write(seat.space.join("branch-issues"), issues.to_string()).unwrap();
    let first = pull(branch, &retained, 11);
    let path = seat.space.join("branch-projections");
    std::fs::write(&path, json!([first.clone()]).to_string()).unwrap();
    for value in [issue("CLOSED", "Feature"), issue("OPEN", "Bug")] {
        std::fs::write(seat.space.join("branch-issues"), states(value).to_string()).unwrap();
        refused(&run(&seat, false), "concord.delivery.branches");
    }
    std::fs::write(seat.space.join("branch-issues"), issues.to_string()).unwrap();
    git(&seat.source, &["branch", branch, "main"]);
    refused(&run(&seat, false), "concord.delivery.branches");
    git(&seat.source, &["branch", "-D", branch]);
    for (field, code) in [
        ("sha", "concord.delivery.branches"),
        ("body", "concord.delivery.branches"),
        ("state", "concord.delivery.provider"),
        ("branch", "concord.delivery.provider"),
        ("head", "concord.delivery.provider"),
        ("base", "concord.delivery.provider"),
    ] {
        let wrong = mutate(&first, field);
        std::fs::write(&path, json!([wrong]).to_string()).unwrap();
        let output = run(&seat, false);
        assert!(!output.status.success(), "accepted mismatch {field}");
        refused(&output, code);
    }
    for response in [
        json!({"next": true, "body": [first.clone()]}),
        json!({"body": {}}),
        json!({"status": 500, "body": {"message": "refused"}}),
    ] {
        std::fs::write(&path, response.to_string()).unwrap();
        refused(&run(&seat, false), "concord.delivery.provider");
    }
    let second = pull(branch, &retained, 12);
    std::fs::write(&path, json!([first.clone(), second]).to_string()).unwrap();
    refused(&run(&seat, false), "concord.delivery.branches");
    std::fs::write(&path, json!([first]).to_string()).unwrap();
    let prepared = run(&seat, false);
    ok(&prepared);
    let plan = serde_json::from_slice::<Value>(&prepared.stdout).unwrap();
    assert!(plan.get("plan").is_some());
    let before = seat.revision();
    std::fs::write(&path, json!([]).to_string()).unwrap();
    let rejected = consume(seat.space, Some(&seat.server), &prepared.stdout);
    refused(&rejected, "concord.delivery.branches");
    assert_eq!(seat.revision(), before);
    assert!(!seat.space.join("pull-state").exists());
    assert_eq!(head(&remote, branch), retained);
}

fn run(seat: &Seat<'_>, handoff: bool) -> std::process::Output {
    prepare(seat.space, &seat.server, &seat.revision(), handoff)
        .output()
        .expect("prepare")
}

fn ok(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn head(root: &std::path::Path, branch: &str) -> String {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", branch])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn states(node: Value) -> Value {
    json!({"data":{"repository":{"i9":node}}})
}
