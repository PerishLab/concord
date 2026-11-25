#[cfg(unix)]
#[path = "seat/spawn.rs"]
mod spawn;
#[cfg(unix)]
#[path = "start/support.rs"]
mod support;

#[cfg(unix)]
mod unix {
    use super::spawn::{self, Reply, Server};
    use super::support::{fact, git, repository, server, success};
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};

    #[test]
    fn auxiliary() {
        let fixture = tempfile::tempdir().expect("fixture");
        let source = estate(fixture.path());
        let foreign = fixture.path().join("foreign");
        git(
            &source,
            &[
                "worktree",
                "add",
                "--detach",
                foreign.to_str().expect("foreign"),
            ],
        );
        let home = fixture.path().join("plumb");
        let staging = home.join("tmp").join("guard-live");
        std::fs::create_dir_all(home.join("tmp")).expect("staging root");
        git(
            &source,
            &[
                "worktree",
                "add",
                "--detach",
                staging.to_str().expect("staging"),
            ],
        );
        let provider = server(fact());
        let mut command = spawn::concord(fixture.path());
        provider.attach(&mut command);
        let output = command
            .env("PLUMB_HOME", &home)
            .args(["--home", fixture.path().to_str().expect("Space")])
            .args([
                "member",
                "start",
                "PerishLab/concord#26",
                "--claim",
                "crates",
                "--revision",
                "0",
            ])
            .output()
            .expect("run Concord");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let audit = spawn::concord(fixture.path())
            .env("PLUMB_HOME", &home)
            .args([
                "--home",
                fixture.path().to_str().unwrap(),
                "--json",
                "audit",
            ])
            .output()
            .unwrap();
        assert!(!audit.status.success());
        let report: Value = serde_json::from_slice(&audit.stdout).unwrap();
        let faults = report["agreement"]["faults"].as_array().unwrap();
        assert_eq!(faults.len(), 1);
        assert_eq!(faults[0]["code"], "integration.worktree.unknown");
        assert_eq!(
            faults[0]["subject"],
            foreign.canonicalize().unwrap().display().to_string()
        );
        assert!(foreign.join("README.md").is_file());
        assert!(
            !report
                .to_string()
                .contains(&staging.canonicalize().unwrap().display().to_string())
        );
        git(
            &source,
            &[
                "worktree",
                "remove",
                "--force",
                foreign.to_str().expect("foreign"),
            ],
        );
        let audit = spawn::concord(fixture.path())
            .env("PLUMB_HOME", &home)
            .args([
                "--home",
                fixture.path().to_str().expect("Space"),
                "--json",
                "audit",
            ])
            .output()
            .expect("run audit");
        let report: Value = serde_json::from_slice(&audit.stdout).expect("audit JSON");
        assert_eq!(report["agreement"]["faults"], json!([]), "{report}");
    }

    #[test]
    fn refusals() {
        let fixture = tempfile::tempdir().expect("fixture");
        estate(fixture.path());
        let unavailable = Server::start(|_| Reply::status(500, json!({"message": "unavailable"})));
        assert_eq!(
            invoke(fixture.path(), &unavailable),
            "concord.member.observe.provider"
        );
        let mut reply = fact();
        reply["data"]["repository"]["rulesets"]["nodes"] = json!([]);
        let missing = server(reply);
        assert_eq!(
            invoke(fixture.path(), &missing),
            "concord.member.observe.rules"
        );
        let mut reply = fact();
        reply["data"]["repository"]["rulesets"]["pageInfo"]["hasNextPage"] = json!(true);
        let truncated = server(reply);
        assert_eq!(
            invoke(fixture.path(), &truncated),
            "concord.member.observe.truncated"
        );
    }

    #[test]
    fn needs() {
        let fixture = tempfile::tempdir().expect("fixture");
        estate(fixture.path());
        let single = labelled(&["needs:revalidation", "kind:bug"], false);
        let error = refusal(fixture.path(), &single);
        assert_eq!(error["code"], "concord.issue.needs");
        let message = error["message"].as_str().expect("message");
        assert!(message.contains("needs:revalidation"), "{message}");
        assert!(!message.contains("kind:bug"), "{message}");
        let double = labelled(&["needs:owner", "needs:revalidation"], false);
        let error = refusal(fixture.path(), &double);
        assert_eq!(error["code"], "concord.issue.needs");
        let message = error["message"].as_str().expect("message");
        assert!(message.contains("needs:owner"), "{message}");
        assert!(message.contains("needs:revalidation"), "{message}");
        let truncated = labelled(&["kind:bug"], true);
        assert_eq!(
            invoke(fixture.path(), &truncated),
            "concord.member.observe.truncated"
        );
        let foreign = labelled(&["kind:bug"], false);
        start(fixture.path(), &foreign);
        let fixture = tempfile::tempdir().expect("fixture");
        estate(fixture.path());
        let bare = labelled(&[], false);
        start(fixture.path(), &bare);
    }

    fn labelled(names: &[&str], truncated: bool) -> Server {
        let mut reply = fact();
        let nodes: Vec<Value> = names.iter().map(|name| json!({"name": name})).collect();
        reply["data"]["repository"]["issue"]["labels"] = json!({
            "nodes": nodes,
            "pageInfo": {"hasNextPage": truncated},
        });
        server(reply)
    }

    fn start(space: &Path, provider: &Server) {
        success(
            space,
            &[
                "member",
                "start",
                "PerishLab/concord#26",
                "--claim",
                "crates",
                "--revision",
                "0",
            ],
            Some(provider),
        );
    }

    fn estate(space: &Path) -> PathBuf {
        success(space, &["issue", "bootstrap"], None);
        let provider = Server::start(|request| {
            if request.path.starts_with("/repos/") {
                return Reply::json(registered());
            }
            Reply::json(attached())
        });
        success(
            space,
            &["issue", "attach", "PerishLab/concord#26"],
            Some(&provider),
        );
        let source = repository(space, "PerishLab/concord");
        success(
            space,
            &[
                "integration",
                "register",
                "PerishLab/concord",
                "--path",
                source.to_str().expect("source path"),
            ],
            Some(&provider),
        );
        source
    }

    fn registered() -> Value {
        json!({
            "id": 1,
            "node_id": "R_concord",
            "full_name": "PerishLab/concord",
            "private": false,
            "default_branch": "main",
        })
    }

    fn attached() -> Value {
        let issue = json!({
            "id": "I_execution",
            "number": 26,
            "title": "Execution",
            "body": "",
            "url": "https://github.com/PerishLab/concord/issues/26",
            "state": "OPEN",
            "reason": null,
            "updated": "one",
            "writable": true,
            "kind": {"name": "Feature"},
            "repository": {"id": "R_concord"},
        });
        let repository = json!({
            "nameWithOwner": "PerishLab/concord",
            "issue": issue,
        });
        json!({"data": {"repository": repository}})
    }

    fn invoke(space: &Path, provider: &Server) -> String {
        refusal(space, provider)["code"]
            .as_str()
            .expect("error code")
            .to_string()
    }

    fn refusal(space: &Path, provider: &Server) -> Value {
        let mut command = spawn::concord(space);
        provider.attach(&mut command);
        let output = command
            .args(["--home", space.to_str().expect("Space"), "--json"])
            .args([
                "member",
                "start",
                "PerishLab/concord#26",
                "--claim",
                "crates",
                "--revision",
                "0",
            ])
            .output()
            .expect("run Concord");
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stderr).expect("error JSON");
        error["error"].clone()
    }
}
