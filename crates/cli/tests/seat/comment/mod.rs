#[cfg(unix)]
mod render;

#[cfg(unix)]
mod unix {
    use super::super::execution::unix::{Facts, ticket};
    use super::super::spawn::{self, Reply, Request, Server};
    use super::render;
    use serde_json::{Value, json};
    use std::io::Write;
    use std::path::Path;
    use std::process::{Output, Stdio};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

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
            (
                "agy",
                vec![("ANTIGRAVITY_CONVERSATION_ID", "agy-session")],
                "agy-session",
            ),
        ] {
            let fixture = tempfile::tempdir().expect("fixture");
            bootstrap(fixture.path());
            let body = render::body("Decision recorded.", agent, session, None);
            let mock = provider("OPEN", &body, Answer::Exact);
            let run = Run {
                server: &mock.server,
                body: "Decision recorded.\n",
                environment: &environment,
                timeout: 10,
            };
            let value = success(invoke(fixture.path(), run));
            assert_eq!(value["comment"]["execution"]["agent"], agent);
            assert_eq!(value["comment"]["execution"]["session"], session);
            assert_eq!(value["comment"]["execution"]["host"], Value::Null);
            let semantics = value["comment"]["semantics"].as_str().expect("semantics");
            assert!(semantics.contains("observations"));
            request(&mock, &body);
        }

        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let body = render::body("Handoff complete.", "codex", "thread-1", Some("lab/host-1"));
        let mock = provider("CLOSED", &body, Answer::Exact);
        let environment = [
            ("CODEX_THREAD_ID", "thread-1"),
            ("CONCORD_HOST_ID", "lab/host-1"),
        ];
        let run = Run {
            server: &mock.server,
            body: "Handoff complete.",
            environment: &environment,
            timeout: 10,
        };
        let value = success(invoke(fixture.path(), run));
        assert_eq!(value["comment"]["execution"]["host"], "lab/host-1");
        request(&mock, &body);
    }

    #[path = "validation.rs"]
    mod validation;

    #[path = "budget.rs"]
    mod budget;

    #[path = "outcomes.rs"]
    mod outcomes;

    #[derive(Clone, Copy)]
    enum Answer {
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
        answer: Answer,
        timeout: u64,
        code: &'a str,
    }

    impl<'a> Case<'a> {
        const fn new(name: &'a str, answer: Answer, timeout: u64, code: &'a str) -> Self {
            Self {
                name,
                answer,
                timeout,
                code,
            }
        }
    }

    struct Run<'a> {
        server: &'a Server,
        body: &'a str,
        environment: &'a [(&'a str, &'a str)],
        timeout: u64,
    }

    struct Mock {
        server: Server,
        requests: Arc<Mutex<Vec<Value>>>,
        hits: Arc<AtomicUsize>,
    }

    impl Mock {
        fn hits(&self) -> usize {
            self.hits.load(Ordering::SeqCst)
        }

        fn requests(&self) -> Vec<Value> {
            self.requests.lock().expect("requests").clone()
        }
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
        run.server.attach(&mut process);
        process
            .args(["--home", space.to_str().expect("root"), "--json"])
            .args([
                "issue",
                "comment",
                "PerishLab/concord#83",
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

    fn guard() -> Mock {
        let hits = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&hits);
        let server = Server::start(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Reply::status(500, json!({"message": "unexpected"}))
        });
        Mock {
            server,
            requests: Arc::new(Mutex::new(Vec::new())),
            hits,
        }
    }

    fn provider(state: &str, body: &str, answer: Answer) -> Mock {
        let state = state.to_owned();
        let body = body.to_owned();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let hits = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&hits);
        let server = Server::start(move |request: Request| {
            count.fetch_add(1, Ordering::SeqCst);
            if request.query().contains("RunsealIssue") {
                let facts = Facts {
                    node: "I_issue",
                    stable: "R_concord",
                    coordinate: "PerishLab/concord",
                    number: 83,
                };
                return Reply::json(ticket(&facts, &state, "Feature", "2026-09-29T00:00:00Z"));
            }
            recorded.lock().expect("requests").push(request.json());
            mutation(&body, answer)
        });
        Mock {
            server,
            requests,
            hits,
        }
    }

    fn mutation(body: &str, answer: Answer) -> Reply {
        match answer {
            Answer::Refused => return Reply::status(422, json!({"message": "refused"})),
            Answer::Malformed => return Reply::raw(200, b"broken".to_vec()),
            Answer::Oversized => return Reply::raw(200, vec![b'x'; 1_100_000]),
            Answer::Limit => return Reply::raw(422, vec![b'x'; 1_100_000]),
            Answer::Timeout => {
                std::thread::sleep(Duration::from_secs(2));
                return Reply::status(500, json!({"message": "timeout"}));
            }
            Answer::Exact | Answer::Escaped | Answer::Different | Answer::Foreign => {}
        }
        let declared = if matches!(answer, Answer::Different) {
            "different"
        } else {
            body
        };
        let url = if matches!(answer, Answer::Foreign) {
            "https://github.com/PerishLab/concord/issues/84#issuecomment-1"
        } else {
            "https://github.com/PerishLab/concord/issues/83#issuecomment-1"
        };
        let node = json!({
            "id": "IC_comment", "url": url, "body": declared,
            "created": "2026-09-29T00:00:01Z", "updated": "2026-09-29T00:00:01Z",
        });
        let edge = json!({"node": node});
        let addition = json!({"subject": {"id": "I_issue"}, "commentEdge": edge});
        let value = json!({"data": {"addComment": addition}});
        if matches!(answer, Answer::Escaped) {
            let escaped: String = declared
                .encode_utf16()
                .map(|unit| format!("\\u{unit:04x}"))
                .collect();
            let text = value.to_string().replace(
                &serde_json::to_string(declared).expect("body"),
                &format!("\"{escaped}\""),
            );
            return Reply::raw(200, text.into_bytes());
        }
        Reply::json(value)
    }

    fn request(mock: &Mock, body: &str) {
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["variables"]["issue"], "I_issue");
        assert_eq!(requests[0]["variables"]["body"], body);
    }
}
