#[cfg(unix)]
pub(super) mod unix {
    use super::super::spawn;
    use serde_json::{Value, json};
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::{Command, Output};

    #[test]
    fn execution() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let provider = fixture.path().join("provider");
        let reply = json!({
            "node": "I_execution",
            "stable": "R_concord",
            "number": 26,
            "url": "https://github.com/PerishLab/concord/issues/26",
            "state": "OPEN",
            "kind": "Feature",
            "updated_at": "one",
        });
        std::fs::write(&provider, format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n"))
            .expect("provider command");
        let mut permissions = std::fs::metadata(&provider)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&provider, permissions).expect("provider mode");
        success(
            fixture.path(),
            &[
                "issue",
                "attach",
                "PerishLab/concord#26",
                "--github-command",
                provider.to_str().expect("provider path"),
            ],
        );
        let source = repository(fixture.path(), "PerishLab/concord");
        let provider = fixture.path().join("repository-provider");
        let reply = json!({
            "node": "R_concord",
            "coordinate": "PerishLab/concord",
            "branch": "main",
        });
        executable(&provider, &format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n"));
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
        let attached = operator(
            fixture.path(),
            &[
                "member",
                "attach",
                "PerishLab/concord#26",
                "--claim",
                "crates",
                "--revision",
                "0",
            ],
            "CODEX_THREAD_ID",
            "codex-one",
        );
        assert!(attached.status.success());
        let body: Value = serde_json::from_slice(&attached.stdout).expect("Member JSON");
        assert_eq!(body["member"]["node"], "I_execution");
        assert!(
            fixture
                .path()
                .join(".issues/I_execution/worktree/.git")
                .is_file()
        );

        let claimed = operator(
            fixture.path(),
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
            success(fixture.path(), &["issue", "show", "PerishLab/concord#26"])["anchor"]["revision"],
            2
        );
    }

    pub(crate) fn repository(root: &Path, coordinate: &str) -> std::path::PathBuf {
        let source = root.join("source");
        std::fs::create_dir(&source).expect("source");
        git(&source, &["init", "-b", "main"]);
        git(&source, &["config", "user.name", "Concord Test"]);
        git(
            &source,
            &["config", "user.email", "concord@example.invalid"],
        );
        std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
        git(&source, &["add", "README.md"]);
        git(&source, &["commit", "-m", "fixture"]);
        git(
            &source,
            &[
                "remote",
                "add",
                "origin",
                &format!("https://github.com/{coordinate}.git"),
            ],
        );
        git(&source, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(
            &source,
            &["branch", "--set-upstream-to=origin/main", "main"],
        );
        source
    }

    fn executable(path: &Path, body: &str) {
        std::fs::write(path, body).expect("provider command");
        let mut permissions = std::fs::metadata(path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(path, permissions).expect("provider mode");
    }

    fn success(space: &Path, arguments: &[&str]) -> Value {
        let output = operator(space, arguments, "NO_CONCORD_AGENT", "none");
        assert!(output.status.success());
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    fn operator(space: &Path, arguments: &[&str], variable: &str, session: &str) -> Output {
        spawn::concord(space)
            .args(["--root", space.to_str().expect("root path"), "--json"])
            .args(arguments)
            .env(variable, session)
            .output()
            .expect("run Concord as operator")
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
