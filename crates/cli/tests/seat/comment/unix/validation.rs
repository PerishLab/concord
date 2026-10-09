use super::*;

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
        let command = tool(fixture.path(), "touch called; exit 99");
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
