use super::*;

#[test]
fn echoes() {
    let overhead = render::body("", "codex", "thread", None).len();
    let boundary = "a".repeat(65_536 - overhead);
    for (prose, answer) in [
        ("Long decision.\n".repeat(2000), Answer::Exact),
        ("\"\\\u{1}<&中🫧".repeat(1500), Answer::Escaped),
        (boundary, Answer::Escaped),
    ] {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let body = render::body(prose.trim_end(), "codex", "thread", None);
        let mock = provider("OPEN", &body, answer);
        success(invoke(
            fixture.path(),
            Run {
                server: &mock.server,
                body: &prose,
                environment: &[("CODEX_THREAD_ID", "thread")],
                timeout: 10,
            },
        ));
        request(&mock, &body);
    }
}

#[test]
fn limits() {
    let overhead = render::body("", "codex", "thread", None).len();
    for length in [65_536 - overhead + 1, 65_537] {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let mock = guard();
        let output = invoke(
            fixture.path(),
            Run {
                server: &mock.server,
                body: &"a".repeat(length),
                environment: &[("CODEX_THREAD_ID", "thread")],
                timeout: 10,
            },
        );
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("comment.limit"));
        assert_eq!(mock.hits(), 0);
    }
}
