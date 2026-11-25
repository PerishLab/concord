use super::super::execution::unix::{Facts, git, text, ticket};
use super::super::spawn::{Reply, Request, Server};
use super::fixture::Seat;
use serde_json::{Value, json};

#[test]
fn refusal() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    let seat = Seat { space };
    seat.success(None, &["issue", "bootstrap"]);
    let auto = Server::start(|_| {
        Reply::json(ticket(
            &Facts {
                node: "I_auto",
                stable: "R_concord",
                coordinate: "PerishLab/concord",
                number: 2,
            },
            "OPEN",
            "Auto",
            "now",
        ))
    });
    let refused = seat.run(Some(&auto), &["issue", "attach", "PerishLab/concord#2"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("concord.issue.automation"));
    assert!(
        !seat
            .run(None, &["issue", "show", "PerishLab/concord#2"])
            .status
            .success()
    );
    let source = seat.setup();
    seat.attach();
    let changed = seat.start("Auto");
    let refused = seat.run(
        Some(&changed),
        &[
            "member",
            "start",
            "PerishLab/concord#1",
            "--claim",
            "Cargo.lock",
            "--revision",
            "0",
        ],
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("concord.issue.automation"));
    assert!(!space.join("workspace/issues/I_human/worktree").exists());
    assert_eq!(
        seat.success(None, &["issue", "show", "PerishLab/concord#1"])["anchor"]["revision"],
        0
    );
    assert_eq!(text(&source, &["branch", "--list", "auto/*"]), "");
}

#[test]
fn observation() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    let seat = Seat { space };
    seat.success(None, &["issue", "bootstrap"]);
    let source = seat.setup();
    seat.attach();
    let auto = space.join("automatic");
    git(
        &source,
        &[
            "worktree",
            "add",
            "-b",
            "auto/2",
            auto.to_str().expect("auto"),
        ],
    );
    std::fs::write(auto.join("README.md"), "unfinished automation\n").expect("dirty Auto");
    assert_eq!(
        seat.success(None, &["audit"])["agreement"]["faults"],
        json!([])
    );
    let start = seat.start("Feature");
    let started = seat.success(
        Some(&start),
        &[
            "member",
            "start",
            "PerishLab/concord#1",
            "--claim",
            "Cargo.lock",
            "--revision",
            "0",
        ],
    );
    assert_eq!(started["member"]["branch"], "feature/1");
    assert_eq!(
        seat.success(None, &["audit"])["agreement"]["faults"],
        json!([])
    );
    let automatic = server(snapshot("I_auto", 2, "Auto"), false);
    let report = seat.success(
        Some(&automatic),
        &["issue", "preflight", "existing", "PerishLab/concord#2"],
    );
    let held = &report["preflight"];
    assert_eq!(held["complete"], true);
    assert_eq!(held["automation"]["holder"], "automation-held");
    assert_eq!(held["automation"]["operation"], "follow");
    assert_eq!(held["execution"]["members"], json!([]));
    assert_eq!(held["execution"]["occupancy"], json!([]));
    assert_eq!(held["observations"][0]["paths"], json!(["Cargo.lock"]));
    assert_eq!(held["observations"][0]["member"]["number"], 1);
    assert_eq!(held["observations"][0]["exclusive"], false);

    let human = server(snapshot("I_human", 1, "Feature"), false);
    let report = seat.success(
        Some(&human),
        &["issue", "preflight", "existing", "PerishLab/concord#1"],
    );
    assert_eq!(report["preflight"]["observations"][0]["issue"]["number"], 2);

    let truncated = server(snapshot("I_auto", 2, "Auto"), true);
    let report = seat.success(
        Some(&truncated),
        &[
            "issue",
            "preflight",
            "existing",
            "PerishLab/concord#2",
            "--max-pages",
            "1",
        ],
    );
    assert_eq!(report["preflight"]["complete"], false);
    assert_eq!(report["preflight"]["diagnostics"][0]["code"], "automation");
}

#[test]
fn projection() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    let seat = Seat { space };
    seat.success(None, &["issue", "bootstrap"]);
    let server = server(snapshot("I_auto", 2, "Auto"), false);
    for (verb, field) in [
        ("brief", "brief"),
        ("ready", "readiness"),
        ("graph", "graph"),
    ] {
        let report = seat.success(Some(&server), &["issue", verb, "PerishLab/concord#2"]);
        let held = if verb == "graph" {
            &report[field]["nodes"][0]
        } else {
            &report[field]
        };
        assert_eq!(held["automation"]["holder"], "automation-held");
        assert_eq!(held["automation"]["operation"], "follow");
        assert_eq!(held["automation"]["state"], "open");
        if verb == "ready" {
            assert_eq!(held["ready"], false);
            assert_eq!(held["checks"]["required_sections"], true);
            assert!(held.get("acceptance").is_none());
        }
    }
}

fn server(graph: Value, more: bool) -> Server {
    Server::start(move |request: Request| {
        if request.method == "GET" {
            let reply = Reply::json(json!([listing()]));
            if more {
                return reply.header(
                    "Link",
                    "<https://api.github.com/repos/PerishLab/concord/issues?state=open&per_page=50&page=2>; rel=\"next\"",
                );
            }
            return reply;
        }
        Reply::json(graph.clone())
    })
}

fn body() -> &'static str {
    "## Operation\nfollow\n\n## Target\nPerishLab/concord\n\n## Change\nLatest packages\n\n## Acceptance\n- [ ] Guard and merge\n\n## Non-goals\nSource repair\n"
}

fn listing() -> Value {
    json!({
        "node_id": "I_auto",
        "number": 2,
        "html_url": "https://github.com/PerishLab/concord/issues/2",
        "repository_url": "https://api.github.com/repos/PerishLab/concord",
        "title": "Follow latest packages",
        "body": body(),
        "updated_at": "2026-09-29T00:00:00Z",
        "state": "open",
        "state_reason": null,
        "type": {"name": "Auto"},
        "user": {"id": 1, "login": "concord-bot"},
        "labels": [],
    })
}

fn snapshot(node: &str, number: i64, kind: &str) -> Value {
    let empty = json!({"nodes": [], "totalCount": 0, "pageInfo": {"hasNextPage": false, "endCursor": null}});
    let raw = json!({
        "id": node,
        "number": number,
        "url": format!("https://github.com/PerishLab/concord/issues/{number}"),
        "title": "Follow latest packages",
        "body": body(),
        "state": "OPEN",
        "updatedAt": "now",
        "issueType": {"name": kind},
        "repository": {"nameWithOwner": "PerishLab/concord"},
        "parent": null,
        "subIssues": empty,
        "blockedBy": empty,
        "blocking": empty,
        "timelineItems": empty,
        "comments": empty,
        "subIssuesSummary": {"total": 0, "completed": 0, "percentCompleted": 0},
    });
    json!({
        "data": {
            "repository": {
                "id": "R_concord",
                "nameWithOwner": "PerishLab/concord",
                "issue": raw,
            }
        }
    })
}
