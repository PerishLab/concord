#[cfg(unix)]
#[path = "issue"]
mod unix {
    mod rebind;
    mod sections;
    use super::super::execution::unix::{Facts, admission, record, repository, ticket};
    use super::super::spawn::{self, Reply, Request, Server};
    use serde_json::{Value, json};
    use std::path::Path;

    #[test]
    fn retirement() {
        let fixture = tempfile::tempdir().expect("fixture");
        let space = fixture.path();
        success(space, None, &["issue", "bootstrap"]);
        let source = repository(space, "PerishLab/concord");
        let server = Server::start(|_| Reply::json(record("R_concord", "PerishLab/concord")));
        success(
            space,
            Some(&server),
            &[
                "integration",
                "register",
                "PerishLab/concord",
                "--path",
                source.to_str().expect("source"),
            ],
        );
        std::fs::rename(&source, space.join("removed")).expect("domain removal");
        let args = [
            "integration",
            "retire",
            "PerishLab/concord",
            "--node",
            "R_concord",
            "--key",
            "1",
        ];
        let preview = success(space, None, &args);
        assert_eq!(preview["retirement"]["applied"], false);
        assert_eq!(
            success(space, None, &["integration", "list"])["integrations"]
                .as_array()
                .expect("list")
                .len(),
            1
        );
        let mut apply = args.to_vec();
        apply.push("--apply");
        assert_eq!(success(space, None, &apply)["retirement"]["applied"], true);
        assert_eq!(
            success(space, None, &["audit"])["agreement"]["faults"],
            json!([])
        );
        assert!(space.join("removed/.git").is_dir());
        assert!(!source.exists());
    }

    #[test]
    fn relationships() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), None, &["issue", "bootstrap"]);
        let setup = Server::start(|request: Request| {
            if request.method == "GET" && request.path == "/repos/PerishLab/concord" {
                return Reply::json(record("R_concord", "PerishLab/concord"));
            }
            let number = request.variables()["number"].as_i64().unwrap_or(0);
            let node = match number {
                68 => "I_root",
                60 => "I_parent",
                69 => "I_child",
                _ => "I_unknown",
            };
            let facts = Facts {
                node,
                stable: "R_concord",
                coordinate: "PerishLab/concord",
                number,
            };
            if request.query().contains("RunsealAdmission") {
                return Reply::json(admission(&facts, "Task"));
            }
            Reply::json(ticket(&facts, "OPEN", "Task", "2026-09-29T00:00:00Z"))
        });
        for number in [68, 60, 69] {
            success(
                fixture.path(),
                Some(&setup),
                &["issue", "attach", &format!("PerishLab/concord#{number}")],
            );
        }
        let source = repository(fixture.path(), "PerishLab/concord");
        success(
            fixture.path(),
            Some(&setup),
            &[
                "integration",
                "register",
                "PerishLab/concord",
                "--path",
                source.to_str().expect("source path"),
            ],
        );
        member(fixture.path(), &setup, "PerishLab/concord#68", "crates/cli");
        member(fixture.path(), &setup, "PerishLab/concord#60", "crates");
        member(fixture.path(), &setup, "PerishLab/concord#69", "docs");

        let server = Server::start(|request: Request| {
            if request.method == "GET" {
                return Reply::json(json!([]));
            }
            Reply::json(reply(false))
        });
        let report = preflight(fixture.path(), &server, 20);
        let result = &report["preflight"];
        assert_eq!(result["schema"], "concord.issue-preflight-existing/v2");
        assert_eq!(result["complete"], true);
        assert_eq!(result["issue"]["node"], "I_root");
        assert_eq!(result["execution"]["members"][0]["claims"][0], "crates/cli");
        assert_eq!(result["pulls"][0]["kind"], "linked-pull");
        assert_eq!(result["pulls"][0]["pull"]["number"], 72);
        for kind in ["parent", "sub-issue", "blocked-by", "blocking"] {
            assert!(relationship(result, kind).is_some(), "missing {kind}");
        }
        let parent = relationship(result, "parent").expect("parent");
        assert_eq!(parent["execution"]["members"][0]["claims"][0], "crates");
        assert_eq!(parent["overlaps"][0]["paths"][0], "crates/cli");
        let child = relationship(result, "sub-issue").expect("child");
        assert_eq!(child["execution"]["members"][0]["claims"][0], "docs");
        assert!(
            child["overlaps"]
                .as_array()
                .expect("intersections")
                .is_empty()
        );

        let truncated = Server::start(|request: Request| {
            if request.method == "GET" {
                return Reply::json(json!([]));
            }
            Reply::json(reply(true))
        });
        let truncated = preflight(fixture.path(), &truncated, 1);
        assert_eq!(truncated["preflight"]["complete"], false);
        assert_eq!(
            truncated["preflight"]["diagnostics"][0]["code"],
            "concord.issue.projection.page-limit"
        );
        let unavailable = Server::start(|_| Reply::status(500, json!({"message": "down"})));
        let unavailable = preflight(fixture.path(), &unavailable, 20);
        assert_eq!(unavailable["preflight"]["complete"], false);
        assert_eq!(
            unavailable["preflight"]["diagnostics"][0]["code"],
            "concord.issue.projection.provider"
        );
    }

    fn relationship<'a>(result: &'a Value, kind: &str) -> Option<&'a Value> {
        result["relationships"]
            .as_array()
            .expect("relationships")
            .iter()
            .find(|relationship| relationship["kind"] == kind)
    }

    fn member(space: &Path, server: &Server, issue: &str, claim: &str) {
        success(
            space,
            Some(server),
            &[
                "member",
                "start",
                issue,
                "--claim",
                claim,
                "--revision",
                "0",
            ],
        );
    }

    fn preflight(space: &Path, server: &Server, pages: u16) -> Value {
        success(
            space,
            Some(server),
            &[
                "issue",
                "preflight",
                "existing",
                "PerishLab/concord#68",
                "--max-pages",
                &pages.to_string(),
            ],
        )
    }

    fn reply(more: bool) -> Value {
        let issue = json!({
            "id": "I_root", "number": 68,
            "url": "https://github.com/PerishLab/concord/issues/68",
            "title": "Explain existing Issue relationships",
            "body": "## Outcome\nExplain native relationships",
            "state": "OPEN", "updatedAt": "2026-09-29T00:00:00Z",
            "issueType": {"name": "Task"},
            "repository": {"nameWithOwner": "PerishLab/concord"},
            "parent": issue("I_parent", 60),
            "subIssues": connection(vec![issue("I_child", 69)], more),
            "subIssuesSummary": {"total": 1, "completed": 0, "percentCompleted": 0},
            "blockedBy": connection(vec![issue("I_blocker", 70)], more),
            "blocking": connection(vec![issue("I_blocked", 71)], more),
            "timelineItems": connection(vec![pull()], more),
            "comments": connection(Vec::<Value>::new(), false),
        });
        json!({
            "data": {
                "repository": {
                    "id": "R_concord",
                    "nameWithOwner": "PerishLab/concord",
                    "issue": issue,
                }
            }
        })
    }

    fn issue(node: &str, number: i64) -> Value {
        json!({
            "id": node, "number": number,
            "url": format!("https://github.com/PerishLab/concord/issues/{number}"),
            "title": format!("Related Issue {number}"), "state": "OPEN",
            "repository": {"nameWithOwner": "PerishLab/concord"},
        })
    }

    fn pull() -> Value {
        json!({"source": {
            "__typename": "PullRequest", "id": "PR_delivery", "number": 72,
            "url": "https://github.com/PerishLab/concord/pull/72",
            "title": "Deliver relationship preflight", "state": "OPEN", "mergedAt": null,
            "updatedAt": "2026-09-29T00:00:00Z",
            "repository": {"nameWithOwner": "PerishLab/concord"}
        }})
    }

    fn connection(nodes: Vec<Value>, more: bool) -> Value {
        json!({
            "totalCount": nodes.len() + usize::from(more),
            "pageInfo": {"hasNextPage": more, "endCursor": more.then_some("next")},
            "nodes": nodes,
        })
    }

    fn success(space: &Path, server: Option<&Server>, arguments: &[&str]) -> Value {
        let mut command = spawn::concord(space);
        command
            .args(["--home", space.to_str().expect("root path"), "--json"])
            .args(arguments)
            .env("CODEX_THREAD_ID", "existing-preflight");
        if let Some(server) = server {
            server.attach(&mut command);
        }
        let output = command.output().expect("run Concord");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }
}
