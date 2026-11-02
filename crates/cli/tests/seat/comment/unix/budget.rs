use super::*;

#[test]
fn echoes() {
    let overhead = render::body("", "codex", "thread", None).len();
    let boundary = "a".repeat(65_536 - overhead);
    for (prose, reply) in [
        ("Long decision.\n".repeat(2000), Reply::Exact),
        ("\"\\\u{1}<&中🫧".repeat(1500), Reply::Escaped),
        (boundary, Reply::Escaped),
    ] {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let body = render::body(prose.trim_end(), "codex", "thread", None);
        let command = provider(fixture.path(), "OPEN", &body, reply);
        success(invoke(
            fixture.path(),
            Run {
                command: &command,
                body: &prose,
                environment: &[("CODEX_THREAD_ID", "thread")],
                timeout: 10,
            },
        ));
        request(fixture.path(), &body, "thread");
        assert_eq!(
            std::fs::read_to_string(fixture.path().join("mutations")).expect("calls"),
            "mutation\n"
        );
    }
}

#[test]
fn limits() {
    let overhead = render::body("", "codex", "thread", None).len();
    for length in [65_536 - overhead + 1, 65_537] {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let called = fixture.path().join("called");
        let command = tool(
            fixture.path(),
            &format!("touch '{}'; exit 99", called.display()),
        );
        let output = invoke(
            fixture.path(),
            Run {
                command: &command,
                body: &"a".repeat(length),
                environment: &[("CODEX_THREAD_ID", "thread")],
                timeout: 10,
            },
        );
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("comment.limit"));
        assert!(!called.exists());
    }
}
