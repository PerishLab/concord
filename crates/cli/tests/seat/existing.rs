#[cfg(unix)]
mod unix {
    use super::super::execution::unix::{Facts, provider, repository};
    use super::super::spawn;
    use serde_json::{Value, json};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    #[test]
    fn retirement() {
        let fixture = tempfile::tempdir().expect("fixture");
        let space = fixture.path();
        success(space, &["issue", "bootstrap"]);
        let source = repository(space, "PerishLab/concord");
        let provider = tool(
            space,
            "repository",
            format!(
                "printf '%s\\n' '{}'",
                json!({
                    "node": "R_concord", "coordinate": "PerishLab/concord", "branch": "main",
                })
            ),
        );
        success(
            space,
            &[
                "integration",
                "register",
                "PerishLab/concord",
                "--path",
                source.to_str().expect("source"),
                "--github-command",
                provider.to_str().expect("provider"),
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
        let preview = success(space, &args);
        assert_eq!(preview["retirement"]["applied"], false);
        assert_eq!(
            success(space, &["integration", "list"])["integrations"]
                .as_array()
                .expect("list")
                .len(),
            1
        );
        let mut apply = args.to_vec();
        apply.push("--apply");
        assert_eq!(success(space, &apply)["retirement"]["applied"], true);
        assert_eq!(success(space, &["audit"])["agreement"]["faults"], json!([]));
        assert!(space.join("removed/.git").is_dir());
        assert!(!source.exists());
    }

    #[test]
    fn relationships() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        for (node, number) in [("I_root", 68), ("I_parent", 60), ("I_child", 69)] {
            attach(fixture.path(), node, number);
        }
        let source = repository(fixture.path(), "PerishLab/concord");
        let provider = tool(
            fixture.path(),
            "repository",
            format!(
                "printf '%s\\n' '{}'",
                json!({
                    "node": "R_concord",
                    "coordinate": "PerishLab/concord",
                    "branch": "main",
                })
            ),
        );
        success(
            fixture.path(),
            &[
                "integration",
                "register",
                "PerishLab/concord",
                "--path",
                source.to_str().expect("source path"),
                "--github-command",
                provider.to_str().expect("provider path"),
            ],
        );
        member(
            fixture.path(),
            "I_root",
            "PerishLab/concord#68",
            "crates/cli",
        );
        member(fixture.path(), "I_parent", "PerishLab/concord#60", "crates");
        member(fixture.path(), "I_child", "PerishLab/concord#69", "docs");

        let provider = tool(fixture.path(), "relationships", reply(false));
        let report = preflight(fixture.path(), &provider, 20);
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

        let truncated = tool(fixture.path(), "truncated-relationships", reply(true));
        let truncated = preflight(fixture.path(), &truncated, 1);
        assert_eq!(truncated["preflight"]["complete"], false);
        assert_eq!(
            truncated["preflight"]["diagnostics"][0]["code"],
            "concord.issue.projection.page-limit"
        );
        let unavailable = tool(fixture.path(), "unavailable-existing", "exit 1".to_string());
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

    fn attach(space: &Path, node: &str, number: i64) {
        let provider = tool(
            space,
            &format!("observe-{number}"),
            observation(node, number),
        );
        success(
            space,
            &[
                "issue",
                "attach",
                &format!("PerishLab/concord#{number}"),
                "--github-command",
                provider.to_str().expect("provider path"),
            ],
        );
    }

    fn member(space: &Path, node: &str, issue: &str, claim: &str) {
        let number = issue
            .rsplit_once('#')
            .and_then(|(_, number)| number.parse().ok())
            .expect("Issue number");
        let provider = provider(
            space,
            Facts {
                node,
                stable: "R_concord",
                coordinate: "PerishLab/concord",
                number,
            },
        );
        success(
            space,
            &[
                "member",
                "start",
                issue,
                "--claim",
                claim,
                "--revision",
                "0",
                "--github-command",
                provider.to_str().expect("start provider"),
            ],
        );
    }

    fn preflight(space: &Path, provider: &Path, pages: u16) -> Value {
        success(
            space,
            &[
                "issue",
                "preflight",
                "existing",
                "PerishLab/concord#68",
                "--github-command",
                provider.to_str().expect("provider path"),
                "--max-pages",
                &pages.to_string(),
            ],
        )
    }

    fn observation(node: &str, number: i64) -> String {
        let value = json!({
            "node": node, "stable": "R_concord", "number": number,
            "url": format!("https://github.com/PerishLab/concord/issues/{number}"),
            "state": "OPEN", "kind": "Task", "updated_at": "2026-09-29T00:00:00Z",
        });
        format!("printf '%s\\n' '{value}'")
    }

    fn reply(more: bool) -> String {
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
        let value = json!({"data": {"repository": {"issue": issue, "issues": connection(Vec::new(), false)}}});
        format!("printf '%s\\n' '{value}'")
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
            "totalCount": nodes.len(),
            "pageInfo": {"hasNextPage": more, "endCursor": more.then_some("next")},
            "nodes": nodes,
        })
    }

    fn tool(root: &Path, name: &str, body: String) -> PathBuf {
        let path = root.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("provider command");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("provider mode");
        path
    }

    fn success(space: &Path, arguments: &[&str]) -> Value {
        let output = spawn::concord(space)
            .args(["--root", space.to_str().expect("root path"), "--json"])
            .args(arguments)
            .env("CODEX_THREAD_ID", "existing-preflight")
            .output()
            .expect("run Concord");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }
}
