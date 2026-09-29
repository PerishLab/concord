#[cfg(unix)]
mod unix {
    use super::super::spawn;
    use serde_json::{Value, json};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Output, Stdio};

    #[test]
    fn lifecycle() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let command = tool(
            fixture.path(),
            "issue",
            reply(
                "I_issue",
                ("PerishLab", "concord", 25),
                ("OPEN", "Feature", "one"),
            ),
        );
        let command = command.to_str().expect("provider path");
        let attached = success(
            fixture.path(),
            &[
                "issue",
                "attach",
                "PerishLab/concord#25",
                "--github-command",
                command,
            ],
        );
        assert_eq!(attached["anchor"]["node"], "I_issue");
        assert_eq!(attached["anchor"]["revision"], 0);
        assert!(attached["anchor"].get("title").is_none());

        let shown = success(fixture.path(), &["issue", "show", "PerishLab/concord#25"]);
        assert_eq!(shown["anchor"]["node"], "I_issue");
        assert!(shown["anchor"].get("state").is_none());

        let prepared = success(
            fixture.path(),
            &[
                "issue",
                "prepare",
                "PerishLab/concord#25",
                "--revision",
                "0",
                "--github-command",
                command,
            ],
        );
        assert_eq!(prepared["plan"]["schema"], "concord.issue-delivery/v1");
        assert_eq!(prepared["plan"]["observation"]["kind"], "Feature");
        assert_eq!(prepared["plan"]["anchor"]["revision"], 0);
        let validated = pipe(
            fixture.path(),
            &["issue", "validate", "--github-command", command],
            prepared["plan"].clone(),
        );
        assert_eq!(validated["anchor"]["revision"], 0);

        std::fs::write(
            command,
            format!(
                "#!/bin/sh\n{}\n",
                reply(
                    "I_issue",
                    ("PerishLab", "concord", 25),
                    ("CLOSED", "Feature", "two")
                )
            ),
        )
        .expect("change provider observation");
        let drift = invoke(
            fixture.path(),
            &["issue", "validate", "--github-command", command],
            prepared["plan"].clone(),
        );
        assert!(!drift.status.success());
        assert!(String::from_utf8_lossy(&drift.stderr).contains("observation_drift"));
        assert_eq!(
            success(fixture.path(), &["issue", "show", "PerishLab/concord#25"])["anchor"]["revision"],
            0
        );
    }

    #[test]
    fn reconcile() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let before = tool(
            fixture.path(),
            "before",
            reply(
                "I_issue",
                ("PerishLab", "concord", 25),
                ("OPEN", "Feature", "one"),
            ),
        );
        success(
            fixture.path(),
            &[
                "issue",
                "attach",
                "PerishLab/concord#25",
                "--github-command",
                before.to_str().expect("before path"),
            ],
        );
        let after = tool(
            fixture.path(),
            "after",
            reply(
                "I_issue",
                ("PerishLab", "plumb", 40),
                ("OPEN", "Task", "two"),
            ),
        );
        let moved = success(
            fixture.path(),
            &[
                "issue",
                "reconcile",
                "PerishLab/concord#25",
                "PerishLab/plumb#40",
                "--revision",
                "0",
                "--github-command",
                after.to_str().expect("after path"),
            ],
        );
        assert_eq!(moved["anchor"]["repository"], "plumb");
        assert_eq!(moved["anchor"]["revision"], 1);

        let mismatch = tool(
            fixture.path(),
            "mismatch",
            reply(
                "I_other",
                ("PerishLab", "plumb", 41),
                ("OPEN", "Task", "three"),
            ),
        );
        let refused = raw(
            fixture.path(),
            &[
                "issue",
                "reconcile",
                "PerishLab/plumb#40",
                "PerishLab/plumb#41",
                "--revision",
                "1",
                "--github-command",
                mismatch.to_str().expect("mismatch path"),
            ],
        );
        assert!(!refused.status.success());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("node_mismatch"));
    }

    #[test]
    fn failures() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        for (name, body, code) in [
            ("missing", "printf 'null\\n'".to_string(), "missing"),
            ("malformed", "printf 'broken\\n'".to_string(), "malformed"),
            ("provider", "exit 1".to_string(), "provider"),
            (
                "untyped",
                reply("I_issue", ("PerishLab", "concord", 25), ("OPEN", "", "one")),
                "type",
            ),
            (
                "coordinate",
                reply(
                    "I_issue",
                    ("PerishLab", "concord", 26),
                    ("OPEN", "Feature", "one"),
                ),
                "coordinate",
            ),
        ] {
            let command = tool(fixture.path(), name, body);
            let output = raw(
                fixture.path(),
                &[
                    "issue",
                    "attach",
                    "PerishLab/concord#25",
                    "--github-command",
                    command.to_str().expect("provider path"),
                ],
            );
            assert!(!output.status.success(), "{name}");
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains(&format!("concord.issue.observe.{code}")),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn reply(node: &str, coordinate: (&str, &str, i64), view: (&str, &str, &str)) -> String {
        let (owner, repository, number) = coordinate;
        let (state, kind, updated) = view;
        let value = json!({
            "node": node,
            "stable": "R_concord",
            "number": number,
            "url": format!("https://github.com/{owner}/{repository}/issues/{number}"),
            "state": state,
            "kind": kind,
            "updated_at": updated,
        });
        format!("printf '%s\\n' '{}'", value)
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

    fn pipe(space: &Path, arguments: &[&str], body: Value) -> Value {
        let output = invoke(space, arguments, body);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    fn invoke(space: &Path, arguments: &[&str], body: Value) -> Output {
        let mut child = spawn::concord(space)
            .args(["--root", space.to_str().expect("root path"), "--json"])
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn Concord");
        child
            .stdin
            .as_mut()
            .expect("Concord stdin")
            .write_all(serde_json::to_string(&body).expect("plan JSON").as_bytes())
            .expect("write plan");
        child.wait_with_output().expect("wait for Concord")
    }
}
