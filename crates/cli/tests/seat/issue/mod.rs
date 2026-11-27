#[cfg(unix)]
mod automation;
#[cfg(unix)]
mod fixture;
mod landing;
#[cfg(unix)]
mod phase;
#[cfg(unix)]
mod transfer;

#[cfg(unix)]
mod unix {
    use super::super::execution::unix::{Facts, ticket};
    use super::super::spawn::{self, Reply, Request, Server};
    use serde_json::{Value, json};
    use std::io::Write;
    use std::path::Path;
    use std::process::{Output, Stdio};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn lifecycle() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), None, &["issue", "bootstrap"]);
        let drifted = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&drifted);
        let server = Server::start(move |_| {
            let (state, updated) = if flag.load(Ordering::SeqCst) {
                ("CLOSED", "two")
            } else {
                ("OPEN", "one")
            };
            Reply::json(ticket(
                &Facts {
                    node: "I_issue",
                    stable: "R_concord",
                    coordinate: "PerishLab/concord",
                    number: 25,
                },
                state,
                "Feature",
                updated,
            ))
        });
        let attached = success(
            fixture.path(),
            Some(&server),
            &["issue", "attach", "PerishLab/concord#25"],
        );
        assert_eq!(attached["anchor"]["node"], "I_issue");
        assert_eq!(attached["anchor"]["revision"], 0);
        assert!(attached["anchor"].get("title").is_none());

        let shown = success(
            fixture.path(),
            None,
            &["issue", "show", "PerishLab/concord#25"],
        );
        assert_eq!(shown["anchor"]["node"], "I_issue");
        assert!(shown["anchor"].get("state").is_none());

        let prepared = success(
            fixture.path(),
            Some(&server),
            &[
                "issue",
                "prepare",
                "PerishLab/concord#25",
                "--revision",
                "0",
            ],
        );
        assert_eq!(prepared["plan"]["schema"], "concord.issue-delivery/v1");
        assert_eq!(prepared["plan"]["observation"]["kind"], "Feature");
        assert_eq!(prepared["plan"]["anchor"]["revision"], 0);
        let validated = pipe(
            fixture.path(),
            &server,
            &["issue", "validate"],
            prepared["plan"].clone(),
        );
        assert_eq!(validated["anchor"]["revision"], 0);

        drifted.store(true, Ordering::SeqCst);
        let drift = invoke(
            fixture.path(),
            &server,
            &["issue", "validate"],
            prepared["plan"].clone(),
        );
        assert!(!drift.status.success());
        assert!(String::from_utf8_lossy(&drift.stderr).contains("observation_drift"));
        assert_eq!(
            success(
                fixture.path(),
                None,
                &["issue", "show", "PerishLab/concord#25"]
            )["anchor"]["revision"],
            0
        );
    }

    #[test]
    fn reconcile() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), None, &["issue", "bootstrap"]);
        let server = Server::start(|request: Request| {
            let variables = request.variables();
            let name = variables["name"].as_str().unwrap_or("");
            let number = variables["number"].as_i64().unwrap_or(0);
            let (facts, kind, updated) = match (name, number) {
                ("concord", 25) => (
                    Facts {
                        node: "I_issue",
                        stable: "R_concord",
                        coordinate: "PerishLab/concord",
                        number,
                    },
                    "Feature",
                    "one",
                ),
                ("plumb", 40) => (
                    Facts {
                        node: "I_issue",
                        stable: "R_concord",
                        coordinate: "PerishLab/plumb",
                        number,
                    },
                    "Task",
                    "two",
                ),
                _ => (
                    Facts {
                        node: "I_other",
                        stable: "R_concord",
                        coordinate: "PerishLab/plumb",
                        number,
                    },
                    "Task",
                    "three",
                ),
            };
            Reply::json(ticket(&facts, "OPEN", kind, updated))
        });
        success(
            fixture.path(),
            Some(&server),
            &["issue", "attach", "PerishLab/concord#25"],
        );
        let moved = success(
            fixture.path(),
            Some(&server),
            &[
                "issue",
                "reconcile",
                "PerishLab/concord#25",
                "PerishLab/plumb#40",
                "--revision",
                "0",
            ],
        );
        assert_eq!(moved["anchor"]["repository"], "plumb");
        assert_eq!(moved["anchor"]["revision"], 1);

        let refused = raw(
            fixture.path(),
            Some(&server),
            &[
                "issue",
                "reconcile",
                "PerishLab/plumb#40",
                "PerishLab/plumb#41",
                "--revision",
                "1",
            ],
        );
        assert!(!refused.status.success());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("node_mismatch"));
    }

    #[test]
    fn failures() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), None, &["issue", "bootstrap"]);
        let base = Facts {
            node: "I_issue",
            stable: "R_concord",
            coordinate: "PerishLab/concord",
            number: 25,
        };
        let mut wrong = ticket(&base, "OPEN", "Feature", "one");
        wrong["data"]["repository"]["issue"]["url"] =
            "https://github.com/PerishLab/concord/issues/26".into();
        for (name, reply, code) in [
            (
                "missing",
                Reply::json(json!({
                    "data": {"repository": {"nameWithOwner": "PerishLab/concord", "issue": null}}
                })),
                "provider",
            ),
            (
                "provider",
                Reply::status(500, json!({"message": "down"})),
                "provider",
            ),
            (
                "malformed",
                Reply::json(ticket(&base, "BROKEN", "Feature", "one")),
                "malformed",
            ),
            (
                "untyped",
                Reply::json(ticket(&base, "OPEN", "", "one")),
                "type",
            ),
            ("coordinate", Reply::json(wrong), "coordinate"),
        ] {
            let status = reply.status;
            let body = reply.body;
            let server = Server::start(move |_| Reply::raw(status, body.clone()));
            let output = raw(
                fixture.path(),
                Some(&server),
                &["issue", "attach", "PerishLab/concord#25"],
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

    fn success(space: &Path, server: Option<&Server>, arguments: &[&str]) -> Value {
        let output = raw(space, server, arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    fn raw(space: &Path, server: Option<&Server>, arguments: &[&str]) -> Output {
        let mut command = spawn::concord(space);
        command
            .args(["--home", space.to_str().expect("root path"), "--json"])
            .args(arguments);
        if let Some(server) = server {
            server.attach(&mut command);
        }
        command.output().expect("run Concord")
    }

    fn pipe(space: &Path, server: &Server, arguments: &[&str], body: Value) -> Value {
        let output = invoke(space, server, arguments, body);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    fn invoke(space: &Path, server: &Server, arguments: &[&str], body: Value) -> Output {
        let mut command = spawn::concord(space);
        server.attach(&mut command);
        let mut child = command
            .args(["--home", space.to_str().expect("root path"), "--json"])
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
