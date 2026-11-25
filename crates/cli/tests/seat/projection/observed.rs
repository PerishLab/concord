#![cfg(unix)]
use super::super::spawn::{self, Reply, Server};
use super::unix::{PLUMB, raw, serve, success};
use serde_json::{Value, json};

#[test]
fn observed() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    success(space, &["issue", "bootstrap"]);
    let server = serve(Some(PLUMB));
    let config = space.join("home").join(".concord");
    std::fs::create_dir_all(&config).expect("config home");
    let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
    let endpoint = format!("http://{}", closed.local_addr().expect("address"));
    drop(closed);
    std::fs::write(
        config.join("concord.toml"),
        format!("[locus]\nenabled = true\nendpoint = \"{endpoint}\"\n"),
    )
    .expect("config");
    let mut command = spawn::concord(space);
    server.attach(&mut command);
    let output = command
        .args(["--home", space.to_str().expect("root path"), "--json"])
        .args(["issue", "brief", "PerishLab/concord#27"])
        .env("CODEX_THREAD_ID", "projection-thread")
        .output()
        .expect("observe projection");
    assert!(output.status.success());
    let refusing = Server::start(|_| Reply::status(403, json!({"message": "forbidden"})));
    raw(
        space,
        Some(&refusing),
        &["issue", "brief", "PerishLab/concord#27"],
    );
    success(space, &["audit"]);
    let atoms = std::fs::read_to_string(space.join("state").join("locus").join("active.jsonl"))
        .expect("Locus buffer")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("Locus atom"))
        .collect::<Vec<_>>();
    let find = |name: &str| atoms.iter().rfind(|atom| atom["payload"]["event"] == name);
    let calls = atoms
        .iter()
        .filter(|atom| atom["payload"]["event"] == "provider.call");
    let outcomes = calls.clone().map(|call| &call["payload"]["outcome"]);
    assert_eq!(
        outcomes.collect::<Vec<_>>().last().copied(),
        Some(&json!("refused"))
    );
    for call in calls.clone() {
        assert_eq!(call["context"]["concord.command"], "issue.brief");
        assert_eq!(call["payload"]["operation"], "issue.snapshot");
        assert!(call["payload"]["duration_ms"].is_u64());
        assert!(
            call["payload"]["site"]
                .as_str()
                .is_some_and(|at| at.contains(".rs:"))
        );
    }
    assert!(calls.clone().any(|call| call["payload"]["outcome"] == "ok"));
    let span = &find("cli.start").expect("audit start")["context"]["locus.span"];
    let traced = atoms.iter().filter(|atom| {
        atom.get("source").is_some() && atom["context"]["concord.command"] == "audit"
    });
    assert!(traced.clone().count() > 0);
    for atom in traced {
        assert_eq!(atom["context"]["concord.command"], "audit");
        assert_ne!(&atom["context"]["locus.span"], span);
    }
    let event = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "provider.projection")
        .expect("projection observation");
    assert_eq!(event["payload"]["shape"], "issue.brief");
    assert_eq!(event["payload"]["outcome"], "fresh");
    assert_eq!(event["context"]["concord.command"], "issue.brief");
    assert_eq!(event["producer"], "concord");
    assert!(event["id"].is_string());
    for absent in ["title", "body", "url", "reply", "credential", "arguments"] {
        assert!(
            calls
                .clone()
                .chain([event])
                .all(|atom| atom["payload"].get(absent).is_none())
        );
    }
}

#[test]
fn refusal() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["issue", "bootstrap"]);
    for (status, expected) in [
        (502, "Some(502)"),
        (401, "Some(401)"),
        (403, "Some(403)"),
        (429, "Some(429)"),
    ] {
        let server = Server::start(move |_| {
            Reply::status(
                status,
                json!({"message": "ghp_secret Authorization: Bearer private https://user:password@example.invalid"}),
            )
            .header("x-github-request-id", "ghp_secret")
        });
        let output = raw(
            fixture.path(),
            Some(&server),
            &["issue", "brief", "PerishLab/concord#296"],
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).expect("JSON refusal");
        assert_eq!(error["error"]["code"], "concord.issue.projection.provider");
        let message = error["error"]["message"].as_str().expect("message");
        assert!(message.contains(expected), "{message}");
        for secret in [
            "ghp_secret",
            "Authorization",
            "private",
            "password",
            "example.invalid",
        ] {
            assert!(!message.contains(secret), "{message}");
        }
        assert!(message.len() < 512);
    }
}

#[test]
fn oversized() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["issue", "bootstrap"]);
    let server = Server::start(|_| Reply::raw(200, vec![b'x'; 1_048_577]));
    let output = raw(
        fixture.path(),
        Some(&server),
        &["issue", "brief", "PerishLab/concord#296"],
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("JSON refusal");
    assert_eq!(error["error"]["code"], "concord.issue.projection.provider");
    let message = error["error"]["message"].as_str().expect("message");
    assert!(message.contains("Read"), "{message}");
    assert!(message.contains("Some(200)"), "{message}");
}
