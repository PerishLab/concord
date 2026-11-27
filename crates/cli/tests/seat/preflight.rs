#[cfg(unix)]
mod unix {
    use super::super::execution::unix::{Facts, ticket};
    use super::super::spawn::{self, Reply, Server};
    use serde_json::{Value, json};
    use std::path::Path;

    #[test]
    fn preflight() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), None, &["issue", "bootstrap"]);
        let observed = Server::start(|_| {
            Reply::json(ticket(
                &Facts {
                    node: "I_candidate",
                    stable: "R_concord",
                    coordinate: "PerishLab/concord",
                    number: 62,
                },
                "OPEN",
                "Feature",
                "2026-09-28T00:00:00Z",
            ))
        });
        success(
            fixture.path(),
            Some(&observed),
            &["issue", "attach", "PerishLab/concord#62"],
        );
        let activity = spawn::concord(fixture.path())
            .args([
                "--home",
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
        let server = Server::start(|_| Reply::json(search(false)));
        let report = run(fixture.path(), Some(&server), "Preflight Issue collisions");
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
        let stale = run(fixture.path(), Some(&server), "Preflight Issue collisions");
        assert_eq!(
            stale["preflight"]["candidates"][0]["execution"]["activity"]["observations"][0]["fresh"],
            false
        );
        let empty = Server::start(|_| Reply::json(reply(false, Vec::new())));
        assert!(
            run(fixture.path(), Some(&empty), "Unrelated proposal")["preflight"]["candidates"]
                .as_array()
                .expect("candidates")
                .is_empty()
        );
        let truncated = Server::start(|_| Reply::json(search(true)));
        let truncated = invoke(
            fixture.path(),
            Some(&truncated),
            "Preflight Issue collisions",
            1,
        );
        assert_eq!(truncated["preflight"]["complete"], false);
        assert_eq!(
            truncated["preflight"]["diagnostics"][0]["code"],
            "truncated"
        );
        let unavailable = Server::start(|_| Reply::status(500, json!({"message": "down"})));
        let unavailable = run(
            fixture.path(),
            Some(&unavailable),
            "Preflight Issue collisions",
        );
        assert_eq!(unavailable["preflight"]["complete"], false);
        assert_eq!(
            unavailable["preflight"]["diagnostics"][0]["code"],
            "provider"
        );
    }

    #[test]
    fn refusal() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), None, &["issue", "bootstrap"]);
        let server = Server::start(|_| Reply::status(500, json!({"message": "unavailable"})));
        let refused = run(fixture.path(), Some(&server), "Preflight Issue collisions");
        assert_eq!(refused["preflight"]["complete"], false);
        assert_eq!(refused["preflight"]["diagnostics"][0]["code"], "provider");
        let missing = run(fixture.path(), None, "Preflight Issue collisions");
        assert_eq!(missing["preflight"]["complete"], false);
        assert_eq!(missing["preflight"]["diagnostics"][0]["code"], "provider");
        let help = spawn::concord(fixture.path())
            .args(["issue", "preflight", "proposed", "--help"])
            .output()
            .expect("help");
        assert!(help.status.success());
        let help = String::from_utf8_lossy(&help.stdout);
        assert!(!help.contains("--github-command"), "{help}");
    }

    fn run(space: &Path, server: Option<&Server>, title: &str) -> Value {
        invoke(space, server, title, 4)
    }

    fn invoke(space: &Path, server: Option<&Server>, title: &str, pages: u16) -> Value {
        success(
            space,
            server,
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
                "--max-pages",
                &pages.to_string(),
            ],
        )
    }

    fn stale(space: &Path) {
        let path = space.join("workspace/activity/issue-1.json");
        let mut ledger: Value = serde_json::from_slice(&std::fs::read(&path).expect("activity"))
            .expect("activity JSON");
        ledger["touches"][0]["time"] = 1.into();
        std::fs::write(path, serde_json::to_vec(&ledger).expect("activity JSON"))
            .expect("write activity");
    }

    fn search(more: bool) -> Value {
        let issue = json!({
            "__typename": "Issue", "id": "I_candidate", "number": 62,
            "url": "https://github.com/PerishLab/concord/issues/62",
            "title": "Preflight Issue collisions",
            "body": "## Outcome\nWarn before duplicate execution", "state": "OPEN",
            "updated": "2026-09-28T00:00:00Z", "kind": {"name": "Feature"},
            "repository": {"name": "PerishLab/concord"},
        });
        let related = json!({
            "__typename": "Issue", "id": "I_related", "number": 66,
            "url": "https://github.com/PerishLab/concord/issues/66",
            "title": "Join GitHub candidates to active work",
            "body": "## Outcome\nReport duplicate execution before allocation", "state": "OPEN",
            "updated": "2026-09-28T00:00:00Z", "kind": {"name": "Task"},
            "repository": {"name": "PerishLab/concord"},
        });
        reply(more, vec![issue, related])
    }

    fn reply(more: bool, rows: Vec<Value>) -> Value {
        let page = json!({"more": more, "cursor": more.then_some("next")});
        let search = json!({"total": rows.len(), "page": page, "rows": rows});
        json!({"data": {"search": search}})
    }

    fn success(space: &Path, server: Option<&Server>, arguments: &[&str]) -> Value {
        let mut command = spawn::concord(space);
        command
            .args(["--home", space.to_str().expect("root path"), "--json"])
            .args(arguments);
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
