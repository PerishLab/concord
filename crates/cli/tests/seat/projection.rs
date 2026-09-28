#[cfg(unix)]
mod unix {
    use super::super::spawn;
    use serde_json::{Value, json};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Output;

    #[test]
    fn projections() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let command = tool(fixture.path(), "projection", reply());
        let command = command.to_str().expect("provider path");
        let brief = success(
            fixture.path(),
            &[
                "issue",
                "brief",
                "PerishLab/concord#27",
                "--github-command",
                command,
                "--page-size",
                "1",
                "--blocking-after",
                "exact-cursor",
            ],
        );
        assert_eq!(brief["brief"]["schema"], "concord.issue-brief/v1");
        assert_eq!(brief["brief"]["kind"], "Feature");
        assert_eq!(
            brief["brief"]["blocking"]["requested_after"],
            "exact-cursor"
        );
        assert_eq!(brief["brief"]["pulls"]["nodes"][0]["number"], 32);
        locus(fixture.path(), command);

        let graph = success(
            fixture.path(),
            &[
                "issue",
                "graph",
                "PerishLab/concord#27",
                "--github-command",
                command,
                "--max-nodes",
                "4",
            ],
        );
        assert_eq!(graph["graph"]["schema"], "concord.issue-graph/v1");
        assert_eq!(graph["graph"]["complete"], true);
        assert_eq!(graph["graph"]["nodes"].as_array().expect("nodes").len(), 1);

        let ready = success(
            fixture.path(),
            &[
                "issue",
                "ready",
                "PerishLab/concord#27",
                "--github-command",
                command,
            ],
        );
        assert_eq!(ready["readiness"]["schema"], "concord.issue-readiness/v1");
        assert_eq!(ready["readiness"]["ready"], true);
        assert_eq!(ready["readiness"]["checks"]["acceptance_total"], 1);
        assert_eq!(
            ready["readiness"]["distribution_evidence"][0],
            "https://releases.plumb.perish.uk/v1/releases/stable/v1/distribution.json"
        );
        unavailable(fixture.path());
    }

    fn unavailable(space: &Path) {
        let command = tool(space, "unavailable-projection", "exit 1".to_string());
        let command = command.to_str().expect("unavailable path");
        let brief = raw(
            space,
            &[
                "issue",
                "brief",
                "PerishLab/concord#27",
                "--github-command",
                command,
            ],
        );
        assert!(!brief.status.success());
        assert!(
            String::from_utf8_lossy(&brief.stderr).contains("concord.issue.projection.provider")
        );
        let graph = success(
            space,
            &[
                "issue",
                "graph",
                "PerishLab/concord#27",
                "--github-command",
                command,
            ],
        );
        assert_eq!(graph["graph"]["complete"], false);
        assert_eq!(graph["graph"]["diagnostics"][0]["code"], "provider");
        assert!(
            graph["graph"]["nodes"]
                .as_array()
                .expect("nodes")
                .is_empty()
        );
    }

    fn locus(space: &Path, provider: &str) {
        let report = space.join("projection.jsonl");
        let output = spawn::concord(space)
            .args(["--root", space.to_str().expect("root path"), "--json"])
            .args([
                "issue",
                "brief",
                "PerishLab/concord#27",
                "--github-command",
                provider,
            ])
            .env("CONCORD_LOCUS_ENABLED", "true")
            .env("CONCORD_LOCUS_REPORT_FILE", &report)
            .env("CODEX_THREAD_ID", "projection-thread")
            .output()
            .expect("observe projection");
        assert!(output.status.success());
        let atoms = std::fs::read_to_string(report).expect("Locus report");
        let event = atoms
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("Locus atom"))
            .find(|atom| atom["payload"]["event"] == "provider.projection")
            .expect("projection observation");
        assert_eq!(event["payload"]["shape"], "issue.brief");
        assert_eq!(event["payload"]["outcome"], "fresh");
        assert_eq!(event["context"]["concord.command"], "issue.brief");
        for absent in ["title", "body", "url", "reply", "credential"] {
            assert!(event["payload"].get(absent).is_none());
        }
    }

    fn reply() -> String {
        let issue = json!({
            "id": "I_issue",
            "number": 27,
            "url": "https://github.com/PerishLab/concord/issues/27",
            "title": "Bounded projection",
            "body": "## Problem\nLedger drift\n\n## Outcome\nProject GitHub\n\n## Acceptance\n- [x] bounded\n\n## Non-goals\nMutation\n",
            "state": "OPEN",
            "updatedAt": "2026-09-28T00:00:00Z",
            "issueType": {"name": "Feature"},
            "repository": {"nameWithOwner": "PerishLab/concord"},
            "parent": null,
            "subIssues": connection(Vec::<Value>::new()),
            "subIssuesSummary": {"total": 0, "completed": 0, "percentCompleted": 0},
            "blockedBy": connection(Vec::<Value>::new()),
            "blocking": connection(Vec::<Value>::new()),
            "timelineItems": connection(vec![pull()]),
            "comments": connection(vec![comment()]),
        });
        let value = json!({"data": {"repository": {"issue": issue}}});
        format!("printf '%s\\n' '{}'", value)
    }

    fn pull() -> Value {
        json!({"source": {
            "__typename": "PullRequest",
            "id": "PR_pull",
            "number": 32,
            "url": "https://github.com/PerishLab/concord/pull/32",
            "title": "Deliver projection",
            "state": "MERGED",
            "mergedAt": "2026-09-28T00:00:00Z",
            "updatedAt": "2026-09-28T00:00:00Z",
            "repository": {"nameWithOwner": "PerishLab/concord"}
        }})
    }

    fn comment() -> Value {
        json!({
            "body": "Released https://releases.plumb.perish.uk/v1/releases/stable/v1/distribution.json",
            "url": "https://github.com/PerishLab/concord/issues/27#issuecomment-1"
        })
    }

    fn connection(nodes: Vec<Value>) -> Value {
        json!({
            "totalCount": nodes.len(),
            "pageInfo": {"hasNextPage": false, "endCursor": "exact-end"},
            "nodes": nodes,
        })
    }

    fn tool(root: &Path, name: &str, body: String) -> PathBuf {
        let path = root.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("provider command");
        let mut permissions = std::fs::metadata(&path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("provider mode");
        path
    }

    fn success(space: &Path, arguments: &[&str]) -> Value {
        let output = raw(space, arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    fn raw(space: &Path, arguments: &[&str]) -> Output {
        spawn::concord(space)
            .args(["--root", space.to_str().expect("root path"), "--json"])
            .args(arguments)
            .output()
            .expect("run Concord")
    }
}
