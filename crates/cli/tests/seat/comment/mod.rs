#[cfg(unix)]
mod render;

#[cfg(unix)]
mod unix {
    use super::super::spawn;
    use super::render;
    use serde_json::{Value, json};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::{Output, Stdio};

    #[test]
    fn declarations() {
        for (agent, environment, session) in [
            (
                "claude",
                vec![("CLAUDE_CODE_SESSION_ID", "claude-session")],
                "claude-session",
            ),
            (
                "grok",
                vec![("GROK_SESSION_ID", "grok-session")],
                "grok-session",
            ),
            (
                "codex",
                vec![("CODEX_THREAD_ID", "codex-session")],
                "codex-session",
            ),
            (
                "grok-bot",
                vec![("GROK_BOT_IDENTITY", "Bubble Santi")],
                "Bubble Santi",
            ),
            (
                "santi",
                vec![("SANTI_SOUL_ID", "soul-1"), ("SANTI_STRAND_ID", "strand-1")],
                "soul-1:strand-1",
            ),
        ] {
            let fixture = tempfile::tempdir().expect("fixture");
            bootstrap(fixture.path());
            let body = render::body("Decision recorded.", agent, session, None);
            let command = provider(fixture.path(), "OPEN", &body, Reply::Exact);
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
            let semantics = value["comment"]["semantics"].as_str().expect("semantics");
            assert!(semantics.contains("observations"));
            request(fixture.path(), &body, session);
        }

        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let body = render::body("Handoff complete.", "codex", "thread-1", Some("lab/host-1"));
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

    #[path = "validation.rs"]
    mod validation;

    #[path = "budget.rs"]
    mod budget;

    #[path = "outcomes.rs"]
    mod outcomes;

    #[derive(Clone, Copy)]
    enum Reply {
        Exact,
        Escaped,
        Refused,
        Malformed,
        Different,
        Foreign,
        Oversized,
        Limit,
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
        assert!(
            spawn::concord(space)
                .args([
                    "--home",
                    space.to_str().expect("root"),
                    "issue",
                    "bootstrap"
                ])
                .output()
                .expect("bootstrap")
                .status
                .success()
        );
    }

    fn invoke(space: &Path, run: Run<'_>) -> Output {
        let timeout = run.timeout.to_string();
        let mut process = spawn::concord(space);
        process
            .args(["--home", space.to_str().expect("root"), "--json"])
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
            "stable": "R_concord",
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
            "url": if matches!(reply, Reply::Foreign) { "https://github.com/PerishLab/concord/issues/84#issuecomment-1" } else { "https://github.com/PerishLab/concord/issues/83#issuecomment-1" },
            "body": declared,
            "createdAt": "2026-09-29T00:00:01Z",
        });
        let edge = json!({"node": node});
        let addition = json!({"commentEdge": edge});
        let answer = json!({"data": {"addComment": addition}});
        let answer = if matches!(reply, Reply::Escaped) {
            let escaped: String = declared
                .encode_utf16()
                .map(|unit| format!("\\u{unit:04x}"))
                .collect();
            answer.to_string().replace(
                &serde_json::to_string(declared).expect("body"),
                &format!("\"{escaped}\""),
            )
        } else {
            answer.to_string()
        };
        let mutation = match reply {
            Reply::Exact | Reply::Escaped | Reply::Different | Reply::Foreign => {
                format!("printf '%s\\n' '{answer}'")
            }
            Reply::Refused => "printf refused >&2; exit 1".to_string(),
            Reply::Malformed => "printf broken".to_string(),
            Reply::Oversized => "yes x | head -c 20000".to_string(),
            Reply::Limit => "yes x | head -c 20000 >&2; exit 1".to_string(),
            Reply::Timeout => "sleep 2".to_string(),
        };
        tool(
            space,
            &format!(
                "case \"$*\" in\n*\"--input -\"*) echo mutation >> '{}'; printf '%s' \"$*\" > '{}'; cat > '{}'; {mutation} ;;\n*) printf '%s\\n' '{observation}' ;;\nesac",
                space.join("mutations").display(),
                arguments.display(),
                request.display(),
            ),
        )
    }

    fn tool(space: &Path, body: &str) -> PathBuf {
        let path = space.join("provider");
        plumb::testing::executable(&path, format!("#!/bin/sh\n{body}\n"));
        path
    }

    fn request(space: &Path, body: &str, session: &str) {
        let request: Value = serde_json::from_slice(
            &std::fs::read(space.join("request.json")).expect("provider request"),
        )
        .expect("request JSON");
        assert_eq!(request["variables"]["subject"], "I_issue");
        assert_eq!(request["variables"]["body"], body);
        let arguments = std::fs::read_to_string(space.join("arguments.txt")).expect("arguments");
        assert_eq!(arguments, "api graphql --input - --hostname github.com");
        assert!(!arguments.contains(session));
    }
}
