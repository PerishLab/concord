#[cfg(unix)]
mod unix {
    use super::super::spawn;
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};

    #[test]
    fn preflight() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let observed = tool(fixture.path(), "candidate", observation("I_candidate", 62));
        success(
            fixture.path(),
            &[
                "issue",
                "attach",
                "PerishLab/concord#62",
                "--github-command",
                observed.to_str().expect("candidate path"),
            ],
        );
        let activity = spawn::concord(fixture.path())
            .args([
                "--space",
                fixture.path().to_str().expect("fixture path"),
                "member",
                "list",
                "--issue",
                "PerishLab/concord#62",
            ])
            .env("CODEX_THREAD_ID", "other-session")
            .output()
            .expect("record activity");
        assert!(activity.status.success());
        let command = tool(fixture.path(), "preflight", search(false));
        let report = run(fixture.path(), &command, "Preflight Issue collisions");
        assert_eq!(report["preflight"]["schema"], "concord.issue-preflight/v2");
        assert_eq!(report["preflight"]["complete"], true);
        assert_eq!(report["preflight"]["candidates"][0]["issue"]["number"], 62);
        assert_eq!(
            report["preflight"]["candidates"][0]["signals"][0],
            "exact-title"
        );
        assert_eq!(
            report["preflight"]["candidates"][0]["execution"]["activity"]["observations"][0]["fresh"],
            true
        );
        assert_eq!(report["preflight"]["candidates"][1]["issue"]["number"], 66);
        assert!(
            report["preflight"]["candidates"][1]["signals"]
                .as_array()
                .expect("signals")
                .iter()
                .any(|signal| signal == "outcome-terms")
        );
        stale(fixture.path());
        let stale = run(fixture.path(), &command, "Preflight Issue collisions");
        assert_eq!(
            stale["preflight"]["candidates"][0]["execution"]["activity"]["observations"][0]["fresh"],
            false
        );
        let empty = tool(fixture.path(), "empty", empty());
        assert!(
            run(fixture.path(), &empty, "Unrelated proposal")["preflight"]["candidates"]
                .as_array()
                .expect("candidates")
                .is_empty()
        );
        let truncated = tool(fixture.path(), "truncated", search(true));
        let truncated = invoke(fixture.path(), &truncated, "Preflight Issue collisions", 1);
        assert_eq!(truncated["preflight"]["complete"], false);
        assert_eq!(
            truncated["preflight"]["diagnostics"][0]["code"],
            "truncated"
        );
        let unavailable = tool(fixture.path(), "unavailable", "exit 1".to_string());
        let unavailable = run(fixture.path(), &unavailable, "Preflight Issue collisions");
        assert_eq!(unavailable["preflight"]["complete"], false);
        assert_eq!(
            unavailable["preflight"]["diagnostics"][0]["code"],
            "provider"
        );
    }

    #[test]
    fn refusal() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let program = tool(fixture.path(), "gh", empty());
        let joined = PathBuf::from(format!("{} issue create", program.display()));
        let report = run(fixture.path(), &joined, "Preflight Issue collisions");
        let diagnostic = &report["preflight"]["diagnostics"][0];
        assert_eq!(report["preflight"]["complete"], false);
        assert_eq!(diagnostic["code"], "command");
        let message = diagnostic["message"].as_str().expect("message");
        assert!(message.contains("--github-command"), "{message}");
        assert!(message.contains(&joined.display().to_string()), "{message}");
        assert!(
            message.contains("Concord supplies every argument"),
            "{message}"
        );
        let refused = tool(fixture.path(), "refused", "exit 1".to_string());
        let refused = run(fixture.path(), &refused, "Preflight Issue collisions");
        assert_eq!(refused["preflight"]["diagnostics"][0]["code"], "provider");
        let help = spawn::concord(fixture.path())
            .args(["issue", "preflight", "proposed", "--help"])
            .output()
            .expect("help");
        let help = String::from_utf8_lossy(&help.stdout);
        assert!(help.contains("--github-command <PROGRAM>"), "{help}");
        assert!(help.contains("Concord supplies every argument"), "{help}");
    }

    fn run(space: &Path, command: &Path, title: &str) -> Value {
        invoke(space, command, title, 4)
    }

    fn invoke(space: &Path, command: &Path, title: &str, pages: u16) -> Value {
        success(
            space,
            &[
                "issue",
                "preflight",
                "proposed",
                "PerishLab/concord",
                "--kind",
                "Feature",
                "--title",
                title,
                "--outcome",
                "Warn before duplicate execution",
                "--github-command",
                command.to_str().expect("provider path"),
                "--max-pages",
                &pages.to_string(),
            ],
        )
    }

    fn stale(space: &Path) {
        let path = space.join(".concord/activity/issue-1.json");
        let mut ledger: Value = serde_json::from_slice(&std::fs::read(&path).expect("activity"))
            .expect("activity JSON");
        ledger["touches"][0]["time"] = 1.into();
        std::fs::write(path, serde_json::to_vec(&ledger).expect("activity JSON"))
            .expect("write activity");
    }

    fn observation(node: &str, number: i64) -> String {
        let value = json!({
            "node": node, "stable": "R_concord", "number": number,
            "url": format!("https://github.com/PerishLab/concord/issues/{number}"),
            "state": "OPEN", "kind": "Feature", "updated_at": "2026-09-28T00:00:00Z",
        });
        format!("printf '%s\\n' '{}'", value)
    }

    fn search(more: bool) -> String {
        let issue = json!({
            "id": "I_candidate", "number": 62,
            "url": "https://github.com/PerishLab/concord/issues/62",
            "title": "Preflight Issue collisions",
            "body": "## Outcome\nWarn before duplicate execution", "state": "OPEN",
            "updatedAt": "2026-09-28T00:00:00Z", "issueType": {"name": "Feature"},
            "repository": {"nameWithOwner": "PerishLab/concord"},
        });
        let related = json!({
            "id": "I_related", "number": 66,
            "url": "https://github.com/PerishLab/concord/issues/66",
            "title": "Join GitHub candidates to active work",
            "body": "## Outcome\nReport duplicate execution before allocation", "state": "OPEN",
            "updatedAt": "2026-09-28T00:00:00Z", "issueType": {"name": "Task"},
            "repository": {"nameWithOwner": "PerishLab/concord"},
        });
        reply(more, vec![issue, related])
    }

    fn empty() -> String {
        reply(false, Vec::new())
    }

    fn reply(more: bool, nodes: Vec<Value>) -> String {
        let page = json!({"hasNextPage": more, "endCursor": more.then_some("next")});
        let search = json!({"issueCount": nodes.len(), "pageInfo": page, "nodes": nodes});
        let value = json!({"data": {"search": search}});
        format!("printf '%s\\n' '{}'", value)
    }

    fn tool(root: &Path, name: &str, body: String) -> PathBuf {
        let path = root.join(name);
        plumb::testing::executable(&path, format!("#!/bin/sh\n{body}\n"));
        path
    }

    fn success(space: &Path, arguments: &[&str]) -> Value {
        let output = spawn::concord(space)
            .args(["--space", space.to_str().expect("root path"), "--json"])
            .args(arguments)
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
