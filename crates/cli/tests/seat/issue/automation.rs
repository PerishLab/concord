use super::super::execution::unix::git;
use super::fixture::Seat;
use serde_json::{Value, json};

#[test]
fn refusal() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    let seat = Seat { space };
    seat.success(&["issue", "bootstrap"]);
    let auto = seat.tool("auto",
        json!({"node":"I_auto", "stable":"R_concord", "number":2, "url":"https://github.com/PerishLab/concord/issues/2", "state":"OPEN", "kind":"Auto", "updated_at":"now"}),
    );
    let refused = seat.run(&[
        "issue",
        "attach",
        "PerishLab/concord#2",
        "--github-command",
        auto.to_str().expect("provider"),
    ]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("concord.issue.automation"));
    assert!(
        !seat
            .run(&["issue", "show", "PerishLab/concord#2"])
            .status
            .success()
    );
    let source = seat.setup();
    seat.attach();
    let command = seat.start();
    let body = std::fs::read_to_string(&command)
        .expect("provider")
        .replace("Feature", "Auto");
    std::fs::write(&command, body).expect("changed provider");
    let refused = seat.run(&[
        "member",
        "start",
        "PerishLab/concord#1",
        "--claim",
        "Cargo.lock",
        "--revision",
        "0",
        "--github-command",
        command.to_str().expect("provider"),
    ]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("concord.issue.automation"));
    assert!(!space.join(".issues/I_human/worktree").exists());
    assert_eq!(
        seat.success(&["issue", "show", "PerishLab/concord#1"])["anchor"]["revision"],
        0
    );
    assert_eq!(
        super::super::execution::unix::text(&source, &["branch", "--list", "auto/*"]),
        ""
    );
}

#[test]
fn observation() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    let seat = Seat { space };
    seat.success(&["issue", "bootstrap"]);
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
    assert_eq!(seat.success(&["audit"])["agreement"]["faults"], json!([]));
    let command = seat.start();
    let started = seat.success(&[
        "member",
        "start",
        "PerishLab/concord#1",
        "--claim",
        "Cargo.lock",
        "--revision",
        "0",
        "--github-command",
        command.to_str().expect("provider"),
    ]);
    assert_eq!(started["member"]["branch"], "feature/1");
    assert_eq!(seat.success(&["audit"])["agreement"]["faults"], json!([]));
    let command = seat.tool("projection", reply(false));
    let report = seat.success(&[
        "issue",
        "preflight",
        "existing",
        "PerishLab/concord#2",
        "--github-command",
        command.to_str().expect("provider"),
    ]);
    let held = &report["preflight"];
    assert_eq!(held["complete"], true);
    assert_eq!(held["automation"]["holder"], "automation-held");
    assert_eq!(held["automation"]["operation"], "follow");
    assert_eq!(held["execution"]["members"], json!([]));
    assert_eq!(held["execution"]["occupancy"], json!([]));
    assert_eq!(held["observations"][0]["paths"], json!(["Cargo.lock"]));
    assert_eq!(held["observations"][0]["member"]["number"], 1);
    assert_eq!(held["observations"][0]["exclusive"], false);
    let mut human = reply(false);
    human["data"]["repository"]["issue"]["id"] = "I_human".into();
    human["data"]["repository"]["issue"]["number"] = 1.into();
    human["data"]["repository"]["issue"]["url"] =
        "https://github.com/PerishLab/concord/issues/1".into();
    human["data"]["repository"]["issue"]["issueType"]["name"] = "Feature".into();
    let command = seat.tool("human-projection", human);
    let report = seat.success(&[
        "issue",
        "preflight",
        "existing",
        "PerishLab/concord#1",
        "--github-command",
        command.to_str().expect("provider"),
    ]);
    assert_eq!(report["preflight"]["observations"][0]["issue"]["number"], 2);
    let command = seat.tool("truncated", reply(true));
    let report = seat.success(&[
        "issue",
        "preflight",
        "existing",
        "PerishLab/concord#2",
        "--max-pages",
        "1",
        "--github-command",
        command.to_str().expect("provider"),
    ]);
    assert_eq!(report["preflight"]["complete"], false);
    assert_eq!(report["preflight"]["diagnostics"][0]["code"], "automation");
}

#[test]
fn projection() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    let seat = Seat { space };
    seat.success(&["issue", "bootstrap"]);
    let command = seat.tool("projection", reply(false));
    for (verb, field) in [
        ("brief", "brief"),
        ("ready", "readiness"),
        ("graph", "graph"),
    ] {
        let report = seat.success(&[
            "issue",
            verb,
            "PerishLab/concord#2",
            "--github-command",
            command.to_str().expect("provider"),
        ]);
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

fn reply(more: bool) -> Value {
    let empty =
        json!({"nodes":[], "totalCount":0, "pageInfo":{"hasNextPage":false,"endCursor":null}});
    let raw = json!({
        "id":"I_auto", "number":2, "url":"https://github.com/PerishLab/concord/issues/2",
        "title":"Follow latest packages", "body":"## Operation\nfollow\n\n## Target\nPerishLab/concord\n\n## Change\nLatest packages\n\n## Acceptance\n- [ ] Guard and merge\n\n## Non-goals\nSource repair\n",
        "state":"OPEN", "updatedAt":"now", "issueType":{"name":"Auto"},
        "repository":{"nameWithOwner":"PerishLab/concord"}, "parent":null,
        "subIssues":empty, "blockedBy":empty, "blocking":empty, "timelineItems":empty, "comments":empty,
        "subIssuesSummary":{"total":0,"completed":0,"percentCompleted":0}
    });
    let page = json!({"hasNextPage":more, "endCursor":more.then_some("next")});
    let issues = json!({"nodes":[raw], "pageInfo":page});
    json!({"data":{"repository":{"issue":raw, "issues":issues}}})
}
