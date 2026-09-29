#[path = "seat/spawn.rs"]
mod spawn;

#[cfg(unix)]
mod unix {
    use super::spawn;
    use serde_json::{Value, json};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    #[test]
    fn refusals() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let issue = tool(
            fixture.path(),
            "issue",
            json!({
                "node": "I_execution", "stable": "R_concord", "number": 26,
                "url": "https://github.com/PerishLab/concord/issues/26",
                "state": "OPEN", "kind": "Feature", "updated_at": "one",
            }),
        );
        success(
            fixture.path(),
            &[
                "issue",
                "attach",
                "PerishLab/concord#26",
                "--github-command",
                issue.to_str().expect("Issue provider"),
            ],
        );
        let source = repository(fixture.path(), "PerishLab/concord");
        let provider = tool(
            fixture.path(),
            "repository",
            json!({
                "node": "R_concord", "coordinate": "PerishLab/concord", "branch": "main",
            }),
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
                provider.to_str().expect("repository provider"),
            ],
        );
        let unavailable = fixture.path().join("unavailable");
        executable(&unavailable, "#!/bin/sh\nexit 1\n");
        assert_eq!(
            invoke(fixture.path(), &unavailable),
            "concord.member.observe.provider"
        );
        let mut reply = fact();
        reply["rulesets"] = json!([]);
        let missing = tool(fixture.path(), "missing", reply);
        assert_eq!(
            invoke(fixture.path(), &missing),
            "concord.member.observe.rules"
        );
        let mut reply = fact();
        reply["truncated"] = json!(true);
        let truncated = tool(fixture.path(), "truncated", reply);
        assert_eq!(
            invoke(fixture.path(), &truncated),
            "concord.member.observe.truncated"
        );
    }

    fn invoke(space: &Path, provider: &Path) -> String {
        let output = spawn::concord(space)
            .args(["--root", space.to_str().expect("Space"), "--json"])
            .args([
                "member",
                "start",
                "PerishLab/concord#26",
                "--claim",
                "crates",
                "--revision",
                "0",
                "--github-command",
                provider.to_str().expect("provider"),
            ])
            .output()
            .expect("run Concord");
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stderr).expect("error JSON");
        error["error"]["code"]
            .as_str()
            .expect("error code")
            .to_string()
    }

    fn fact() -> Value {
        json!({
            "node": "I_execution", "stable": "R_concord",
            "coordinate": "PerishLab/concord", "branch": "main", "number": 26,
            "url": "https://github.com/PerishLab/concord/issues/26",
            "state": "OPEN", "kind": "Feature", "leaves": 0, "truncated": false,
            "rulesets": [{
                "enforcement": "ACTIVE", "target": "BRANCH",
                "include": ["~DEFAULT_BRANCH"], "exclude": [], "bypass": 0,
                "truncated": false,
                "rules": ["DELETION", "NON_FAST_FORWARD", "PULL_REQUEST"],
            }],
        })
    }

    fn tool(root: &Path, name: &str, reply: Value) -> PathBuf {
        let path = root.join(name);
        executable(&path, &format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n"));
        path
    }

    fn executable(path: &Path, body: &str) {
        std::fs::write(path, body).expect("provider command");
        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(path, permissions).expect("provider mode");
    }

    fn repository(root: &Path, coordinate: &str) -> PathBuf {
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
        std::fs::write(source.join("README.md"), "fixture\n").expect("fixture");
        git(&source, &["add", "README.md"]);
        git(&source, &["commit", "-m", "fixture"]);
        let github = format!("https://github.com/{coordinate}.git");
        git(&source, &["remote", "add", "origin", &github]);
        git(
            &source,
            &[
                "config",
                &format!("url.{}.insteadOf", remote.display()),
                &github,
            ],
        );
        git(&source, &["push", "-u", "origin", "main"]);
        source
    }

    fn git(root: &Path, arguments: &[&str]) {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(arguments)
                .status()
                .expect("run Git")
                .success()
        );
    }

    fn success(space: &Path, arguments: &[&str]) -> Value {
        let output = spawn::concord(space)
            .args(["--root", space.to_str().expect("Space"), "--json"])
            .args(arguments)
            .output()
            .expect("run Concord");
        assert!(output.status.success());
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }
}
