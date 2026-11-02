use super::*;

#[test]
fn outcomes() {
    for case in [
        Case::new("refused", Reply::Refused, 10, "comment.provider"),
        Case::new("malformed", Reply::Malformed, 10, "comment.disagreement"),
        Case::new("different", Reply::Different, 10, "comment.disagreement"),
        Case::new("foreign", Reply::Foreign, 10, "comment.disagreement"),
        Case::new("oversized", Reply::Oversized, 10, "comment.indeterminate"),
        Case::new("large-refusal", Reply::Limit, 10, "comment.indeterminate"),
        Case::new("timeout", Reply::Timeout, 1, "comment.indeterminate"),
    ] {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let body = render::body("Declaration", "codex", "thread", None);
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
        assert_eq!(
            std::fs::read_to_string(fixture.path().join("mutations")).expect("calls"),
            "mutation\n"
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("\"comment\""));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(case.code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
