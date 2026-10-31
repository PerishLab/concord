#![cfg(unix)]
use super::super::spawn;
use super::unix::{PLUMB, answer, raw, reply, success, tool};
use serde_json::{Value, json};

#[test]
fn observed() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    success(space, &["issue", "bootstrap"]);
    let provider = tool(space, "projection", reply(&answer(PLUMB)));
    let provider = provider.to_str().expect("provider path");
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
    let output = spawn::concord(space)
        .args(["--space", space.to_str().expect("root path"), "--json"])
        .args([
            "issue",
            "brief",
            "PerishLab/concord#27",
            "--github-command",
            provider,
        ])
        .env("CODEX_THREAD_ID", "projection-thread")
        .output()
        .expect("observe projection");
    assert!(output.status.success());
    let refusing = tool(space, "refusing", "exit 1".into());
    let refusing = refusing.to_str().expect("refusing path");
    raw(
        space,
        &[
            "issue",
            "brief",
            "PerishLab/concord#27",
            "--github-command",
            refusing,
        ],
    );
    success(space, &["audit"]);
    let atoms = std::fs::read_to_string(config.join("state").join("locus").join("active.jsonl"))
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
        assert_eq!(call["payload"]["operation"], "api graphql");
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
    for (index, (stderr, expected)) in [
        ("gh: Bad Gateway (HTTP 502)", "reported HTTP status 502"),
        ("gh: unauthorized (HTTP 401)", "reported HTTP status 401"),
        ("dial tcp: no such host", "DNS lookup failure"),
        ("net/http: TLS handshake timeout", "TLS handshake timeout"),
        ("read tcp: unexpected EOF", "unexpected EOF"),
        ("HTTP 4010", "cause unknown"),
        ("HTTP 999", "cause unknown"),
        ("", "cause unknown"),
    ]
    .into_iter()
    .enumerate()
    {
        let provider = tool(
            fixture.path(),
            &format!("refusal-{index}"),
            format!(
                "printf '%s\\n' '{stderr} ghp_secret Authorization: Bearer private https://user:password@example.invalid' >&2\nexit 7"
            ),
        );
        let output = raw(
            fixture.path(),
            &[
                "issue",
                "brief",
                "PerishLab/concord#296",
                "--github-command",
                provider.to_str().expect("provider"),
            ],
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).expect("JSON refusal");
        assert_eq!(error["error"]["code"], "concord.issue.projection.provider");
        let message = error["error"]["message"].as_str().expect("message");
        for known in ["api graphql", "exit status: 7", expected] {
            assert!(message.contains(known), "{message}");
        }
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
    let provider = tool(
        fixture.path(),
        "oversized",
        "head -c 1048577 /dev/zero >&2\nexit 7".into(),
    );
    let output = raw(
        fixture.path(),
        &[
            "issue",
            "brief",
            "PerishLab/concord#296",
            "--github-command",
            provider.to_str().expect("provider"),
        ],
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("JSON refusal");
    assert_eq!(error["error"]["code"], "concord.issue.projection.provider");
    assert_eq!(
        error["error"]["message"],
        "GitHub Issue projection refusal exceeds the reply limit"
    );
}
