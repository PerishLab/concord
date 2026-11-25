use super::*;

#[test]
fn outcomes() {
    for case in [
        Case::new("refused", Answer::Refused, 10, "comment.provider"),
        Case::new("malformed", Answer::Malformed, 10, "comment.indeterminate"),
        Case::new("different", Answer::Different, 10, "comment.indeterminate"),
        Case::new("foreign", Answer::Foreign, 10, "comment.disagreement"),
        Case::new("oversized", Answer::Oversized, 10, "comment.indeterminate"),
        Case::new("large-refusal", Answer::Limit, 10, "comment.indeterminate"),
        Case::new("timeout", Answer::Timeout, 1, "comment.indeterminate"),
    ] {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let body = render::body("Declaration", "codex", "thread", None);
        let mock = provider("OPEN", &body, case.answer);
        let environment = [("CODEX_THREAD_ID", "thread")];
        let output = invoke(
            fixture.path(),
            Run {
                server: &mock.server,
                body: "Declaration",
                environment: &environment,
                timeout: case.timeout,
            },
        );
        assert!(!output.status.success(), "{}", case.name);
        assert_eq!(mock.requests().len(), 1);
        assert!(!String::from_utf8_lossy(&output.stdout).contains("\"comment\""));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(case.code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
