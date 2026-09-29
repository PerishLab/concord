#[cfg(unix)]
mod unix {
    use super::super::spawn;
    use serde_json::{Value, json};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Output, Stdio};

    #[test]
    fn declarations() {
        for (variable, agent) in [
            ("CLAUDE_CODE_SESSION_ID", "claude"),
            ("GROK_SESSION_ID", "grok"),
            ("CODEX_THREAD_ID", "codex"),
        ] {
            let fixture = tempfile::tempdir().expect("fixture");
            bootstrap(fixture.path());
            let session = format!("{agent}-session");
            let body = rendered("Decision recorded.", agent, &session, None);
            let command = provider(fixture.path(), "OPEN", &body, Reply::Exact);
            let environment = [(variable, session.as_str())];
            let output = invoke(
                fixture.path(),
                Run {
                    command: &command,
                    body: "Decision recorded.\n",
                    environment: &environment,
                    timeout: 10,
                },
            );
            let value = success(output);
            assert_eq!(value["comment"]["execution"]["agent"], agent);
            assert_eq!(value["comment"]["execution"]["session"], session);
            assert_eq!(value["comment"]["execution"]["host"], Value::Null);
            request(fixture.path(), &body, &session);
        }

        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let body = rendered("Handoff complete.", "codex", "thread-1", Some("lab/host-1"));
        let command = provider(fixture.path(), "CLOSED", &body, Reply::Exact);
        let environment = [
            ("CODEX_THREAD_ID", "thread-1"),
            ("CONCORD_HOST_ID", "lab/host-1"),
        ];
        let value = success(invoke(
            fixture.path(),
            Run {
                command: &command,
                body: "Handoff complete.",
                environment: &environment,
                timeout: 10,
            },
        ));
        assert_eq!(value["comment"]["execution"]["host"], "lab/host-1");
        request(fixture.path(), &body, "thread-1");
    }

    #[test]
    fn validation() {
        for (name, environment, code) in [
            ("missing", vec![], "concord.issue.comment.operator"),
            (
                "ambiguous",
                vec![
                    ("CODEX_THREAD_ID", "thread"),
                    ("GROK_SESSION_ID", "session"),
                ],
                "concord.issue.comment.operator",
            ),
            (
                "host",
                vec![
                    ("CODEX_THREAD_ID", "thread"),
                    ("CONCORD_HOST_ID", "not allowed"),
                ],
                "concord.host.invalid",
            ),
        ] {
            let fixture = tempfile::tempdir().expect("fixture");
            bootstrap(fixture.path());
            let command = refusing(fixture.path(), name);
            let output = invoke(
                fixture.path(),
                Run {
                    command: &command,
                    body: "Declaration",
                    environment: &environment,
                    timeout: 10,
                },
            );
            assert!(!output.status.success(), "{name}");
            assert!(String::from_utf8_lossy(&output.stderr).contains(code));
            assert!(!fixture.path().join("called").exists());
        }
    }

    #[test]
    fn outcomes() {
        for case in [
            Case::new("refused", Reply::Refused, 10, "comment.provider"),
            Case::new("malformed", Reply::Malformed, 10, "comment.disagreement"),
            Case::new("different", Reply::Different, 10, "comment.disagreement"),
            Case::new("timeout", Reply::Timeout, 1, "comment.indeterminate"),
        ] {
            let fixture = tempfile::tempdir().expect("fixture");
            bootstrap(fixture.path());
            let body = rendered("Declaration", "codex", "thread", None);
            let command = provider(fixture.path(), "OPEN", &body, case.reply);
            let environment = [("CODEX_THREAD_ID", "thread")];
            let output = invoke(
                fixture.path(),
                Run {
                    command: &command,
                    body: "Declaration",
                    environment: &environment,
                    timeout: case.timeout,
                },
            );
            assert!(!output.status.success(), "{}", case.name);
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(case.code),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[derive(Clone, Copy)]
    enum Reply {
        Exact,
        Refused,
        Malformed,
        Different,
        Timeout,
    }

    struct Case<'a> {
        name: &'a str,
        reply: Reply,
        timeout: u64,
        code: &'a str,
    }

    impl<'a> Case<'a> {
        const fn new(name: &'a str, reply: Reply, timeout: u64, code: &'a str) -> Self {
            Self {
                name,
                reply,
                timeout,
                code,
            }
        }
    }

    struct Run<'a> {
        command: &'a Path,
        body: &'a str,
        environment: &'a [(&'a str, &'a str)],
        timeout: u64,
    }

    fn bootstrap(space: &Path) {
        let output = spawn::concord(space)
            .args([
                "--root",
                space.to_str().expect("root"),
                "issue",
                "bootstrap",
            ])
            .output()
            .expect("bootstrap");
        assert!(output.status.success());
    }

    fn invoke(space: &Path, run: Run<'_>) -> Output {
        let timeout = run.timeout.to_string();
        let mut process = spawn::concord(space);
        process
            .args(["--root", space.to_str().expect("root"), "--json"])
            .args([
                "issue",
                "comment",
                "PerishLab/concord#83",
                "--github-command",
                run.command.to_str().expect("provider path"),
                "--observe-timeout",
                &timeout,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in run.environment {
            process.env(name, value);
        }
        let mut child = process.spawn().expect("spawn Concord");
        child
            .stdin
            .as_mut()
            .expect("Concord stdin")
            .write_all(run.body.as_bytes())
            .expect("write body");
        child.wait_with_output().expect("wait for Concord")
    }

    fn success(output: Output) -> Value {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    fn provider(space: &Path, state: &str, body: &str, reply: Reply) -> PathBuf {
        let request = space.join("request.json");
        let arguments = space.join("arguments.txt");
        let observation = json!({
            "node": "I_issue",
            "number": 83,
            "url": "https://github.com/PerishLab/concord/issues/83",
            "state": state,
            "kind": "Feature",
            "updated_at": "2026-09-29T00:00:00Z",
        });
        let declared = if matches!(reply, Reply::Different) {
            "different"
        } else {
            body
        };
        let node = json!({
            "id": "IC_comment",
            "url": "https://github.com/PerishLab/concord/issues/83#issuecomment-1",
            "body": declared,
            "createdAt": "2026-09-29T00:00:01Z",
        });
        let edge = json!({"node": node});
        let addition = json!({"commentEdge": edge});
        let answer = json!({"data": {"addComment": addition}});
        let mutation = match reply {
            Reply::Exact | Reply::Different => format!("printf '%s\\n' '{answer}'"),
            Reply::Refused => "printf refused >&2; exit 1".to_string(),
            Reply::Malformed => "printf broken".to_string(),
            Reply::Timeout => "sleep 2".to_string(),
        };
        tool(
            space,
            &format!(
                "case \"$*\" in\n*\"--input -\"*) printf '%s' \"$*\" > '{}'; cat > '{}'; {mutation} ;;\n*) printf '%s\\n' '{observation}' ;;\nesac",
                arguments.display(),
                request.display(),
            ),
        )
    }

    fn refusing(space: &Path, name: &str) -> PathBuf {
        tool(
            space,
            &format!("touch '{}/called'; exit 99 # {name}", space.display()),
        )
    }

    fn tool(space: &Path, body: &str) -> PathBuf {
        let path = space.join("provider");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("provider command");
        let mut permissions = std::fs::metadata(&path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("provider mode");
        path
    }

    fn rendered(prose: &str, agent: &str, session: &str, host: Option<&str>) -> String {
        let value = host
            .map(|host| format!("\"{host}\""))
            .unwrap_or_else(|| "null".to_string());
        let execution =
            format!("{{\"agent\":\"{agent}\",\"session\":\"{session}\",\"host\":{value}}}");
        let host = host
            .map(|host| format!(" · host `{host}`"))
            .unwrap_or_default();
        format!(
            "{prose}\n\n---\nConcord: `{agent}` session `{session}`{host}\n\n<!-- concord.issue-comment/v1\n{execution}\n-->"
        )
    }

    fn request(space: &Path, body: &str, session: &str) {
        let request: Value = serde_json::from_slice(
            &std::fs::read(space.join("request.json")).expect("provider request"),
        )
        .expect("request JSON");
        assert_eq!(request["variables"]["subject"], "I_issue");
        assert_eq!(request["variables"]["body"], body);
        let arguments = std::fs::read_to_string(space.join("arguments.txt")).expect("arguments");
        assert_eq!(arguments, "api graphql --input -");
        assert!(!arguments.contains(session));
    }
}
