#[cfg(unix)]
#[path = "issue/recovery.rs"]
mod recovery;
#[cfg(unix)]
pub(super) mod unix {
    use super::super::spawn::{self, Reply, Request, Server};
    use serde_json::{Value, json};
    use std::path::Path;
    use std::process::{Command, Output};

    #[test]
    fn execution() {
        let fixture = tempfile::tempdir().expect("fixture");
        let root = fixture.path();
        success(root, None, &["issue", "bootstrap"]);
        let server = Server::start(|request: Request| {
            if request.method == "GET" && request.path == "/repos/PerishLab/concord" {
                return Reply::json(record("R_concord", "PerishLab/concord"));
            }
            let facts = Facts {
                node: "I_execution",
                stable: "R_concord",
                coordinate: "PerishLab/concord",
                number: 26,
            };
            if request.query().contains("RunsealAdmission") {
                return Reply::json(admission(&facts, "Feature"));
            }
            Reply::json(ticket(&facts, "OPEN", "Feature", "one"))
        });
        success(
            root,
            Some(&server),
            &["issue", "attach", "PerishLab/concord#26"],
        );
        let source = repository(root, "PerishLab/concord");
        success(
            root,
            Some(&server),
            &[
                "integration",
                "register",
                "PerishLab/concord",
                "--path",
                source.to_str().expect("source path"),
            ],
        );
        let pending = root.join("workspace/issues/I_execution/worktree");
        std::fs::create_dir_all(pending.parent().expect("Issue seat")).expect("Issue seat");
        let branch = "feature/26".to_string();
        git(
            &source,
            &[
                "worktree",
                "add",
                "-b",
                &branch,
                pending.to_str().expect("pending worktree"),
            ],
        );
        let arguments = [
            "member",
            "start",
            "PerishLab/concord#26",
            "--claim",
            "crates",
            "--revision",
            "0",
        ];
        let actor = ("CODEX_THREAD_ID", "codex-one");
        let attached = remote(root, Some(&server), &arguments, actor);
        assert!(
            attached.status.success(),
            "{}",
            String::from_utf8_lossy(&attached.stderr)
        );
        let body: Value = serde_json::from_slice(&attached.stdout).expect("Member JSON");
        assert_eq!(body["member"]["node"], "I_execution");
        assert!(pending.join(".git").is_file());
        let replay = remote(root, Some(&server), &arguments, actor);
        assert!(replay.status.success());
        assert_eq!(
            success(root, None, &["issue", "show", "PerishLab/concord#26"])["anchor"]["revision"],
            1
        );
        let claimed = operator(
            root,
            &[
                "member",
                "claim",
                "PerishLab/concord#26",
                "--claim",
                "docs",
                "--revision",
                "1",
            ],
            "CLAUDE_CODE_SESSION_ID",
            "claude-one",
        );
        assert!(claimed.status.success());
        let warnings = String::from_utf8_lossy(&claimed.stderr)
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("warning JSON"))
            .collect::<Vec<_>>();
        assert!(warnings.iter().any(|warning| {
            warning["warning"]["code"] == "concord.activity.concurrent_session"
                && warning["warning"]["node"] == "I_execution"
        }));
        let occupancy = warnings
            .iter()
            .find(|warning| warning["warning"]["code"] == "concord.occupancy.concurrent_session")
            .expect("occupancy warning");
        let subjects = occupancy["warning"]["conflicts"][0]["subjects"]
            .as_array()
            .expect("conflict subjects");
        for kind in ["issue", "issue-member", "surface"] {
            assert!(subjects.iter().any(|subject| subject["kind"] == kind));
        }
        assert_eq!(
            success(root, None, &["issue", "show", "PerishLab/concord#26"])["anchor"]["revision"],
            2
        );
        super::recovery::run(root, &source, &pending);
        let completed = success(
            root,
            None,
            &[
                "member",
                "complete",
                "PerishLab/concord#26",
                "--revision",
                "2",
                "--apply",
            ],
        );
        assert_eq!(completed["revision"], 3);
        assert!(!pending.exists());
        let mut arguments = arguments;
        arguments[6] = "3";
        let restarted = success(root, Some(&server), &arguments);
        assert_eq!(restarted["member"]["branch"], branch);
        assert!(pending.join(".git").is_file());
        assert_eq!(
            success(root, None, &["issue", "show", "PerishLab/concord#26"])["anchor"]["revision"],
            4
        );
        assert!(
            success(root, None, &["audit"])["agreement"]["faults"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    pub(crate) fn repository(root: &Path, coordinate: &str) -> std::path::PathBuf {
        let source = root.join("source");
        let remote = root.join("remote.git");
        std::fs::create_dir(&source).expect("source");
        std::fs::create_dir(&remote).expect("remote");
        git(&remote, &["init", "--bare"]);
        git(&source, &["init", "-b", "main"]);
        git(&source, &["config", "user.name", "Concord Test"]);
        git(
            &source,
            &["config", "user.email", "concord@example.invalid"],
        );
        std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
        git(&source, &["add", "README.md"]);
        git(&source, &["commit", "-m", "fixture"]);
        let origin = format!("https://github.com/{coordinate}.git");
        git(&source, &["remote", "add", "origin", &origin]);
        git(
            &source,
            &[
                "config",
                &format!("url.{}.insteadOf", remote.display()),
                &origin,
            ],
        );
        git(&source, &["push", "-u", "origin", "main"]);
        source
    }

    pub(crate) struct Facts<'a> {
        pub node: &'a str,
        pub stable: &'a str,
        pub coordinate: &'a str,
        pub number: i64,
    }

    pub(crate) fn record(node: &str, coordinate: &str) -> Value {
        json!({
            "id": 1, "node_id": node, "full_name": coordinate,
            "private": false, "default_branch": "main",
        })
    }

    pub(crate) fn ticket(facts: &Facts<'_>, state: &str, kind: &str, updated: &str) -> Value {
        let label = (!kind.is_empty()).then(|| json!({"name": kind}));
        let url = format!(
            "https://github.com/{}/issues/{}",
            facts.coordinate, facts.number
        );
        let issue = json!({
            "id": facts.node, "number": facts.number, "title": format!("Issue {}", facts.number),
            "body": "Body", "url": url, "state": state, "reason": null,
            "updated": updated, "writable": true, "kind": label,
            "repository": {"id": facts.stable},
        });
        let seat = json!({"nameWithOwner": facts.coordinate, "issue": issue});
        json!({"data": {"repository": seat}})
    }

    pub(crate) fn admission(facts: &Facts<'_>, kind: &str) -> Value {
        let url = format!(
            "https://github.com/{}/issues/{}",
            facts.coordinate, facts.number
        );
        let page = json!({"hasNextPage": false});
        let issue = json!({
            "id": facts.node, "number": facts.number, "url": url, "state": "OPEN",
            "issueType": {"name": kind}, "subIssuesSummary": {"total": 0},
            "labels": {"nodes": [], "pageInfo": page},
        });
        let kinds = [
            json!({"type": "DELETION"}),
            json!({"type": "NON_FAST_FORWARD"}),
            json!({"type": "PULL_REQUEST"}),
        ];
        let rules = json!({"nodes": kinds, "pageInfo": page});
        let names = json!({"include": ["~DEFAULT_BRANCH"], "exclude": []});
        let ruleset = json!({
            "enforcement": "ACTIVE", "target": "BRANCH", "conditions": {"refName": names},
            "bypassActors": {"totalCount": 0}, "rules": rules,
        });
        let rulesets = json!({"nodes": [ruleset], "pageInfo": page});
        let seat = json!({
            "id": facts.stable, "nameWithOwner": facts.coordinate,
            "defaultBranchRef": {"name": "main"}, "issue": issue, "rulesets": rulesets,
        });
        json!({"data": {"repository": seat}})
    }

    pub(super) fn success(space: &Path, server: Option<&Server>, arguments: &[&str]) -> Value {
        let output = remote(space, server, arguments, ("NO_CONCORD_AGENT", "none"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    pub(super) fn operator(
        space: &Path,
        arguments: &[&str],
        variable: &str,
        session: &str,
    ) -> Output {
        remote(space, None, arguments, (variable, session))
    }

    fn remote(
        space: &Path,
        server: Option<&Server>,
        arguments: &[&str],
        actor: (&str, &str),
    ) -> Output {
        let mut command = spawn::concord(space);
        command
            .args(["--home", space.to_str().expect("root path"), "--json"])
            .args(arguments)
            .env(actor.0, actor.1);
        if let Some(server) = server {
            server.attach(&mut command);
        }
        command.output().expect("run Concord as operator")
    }

    pub(crate) fn git(root: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .status()
            .expect("run Git");
        assert!(status.success());
    }

    pub(crate) fn text(root: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .output()
            .expect("run Git");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}
