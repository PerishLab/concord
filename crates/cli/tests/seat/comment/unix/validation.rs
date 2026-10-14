use super::*;

#[test]
fn validation() {
    let long = "a".repeat(129);
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
            "pair",
            vec![
                ("GROK_BOT_IDENTITY", "Bubble Santi"),
                ("GROK_SESSION_ID", "session"),
            ],
            "concord.issue.comment.operator",
        ),
        (
            "santi",
            vec![
                ("GROK_BOT_IDENTITY", "Bubble Santi"),
                ("SANTI_SOUL_ID", "soul"),
                ("SANTI_STRAND_ID", "strand"),
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
    ]
    .into_iter()
    .chain(identities(&long))
    {
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

type Refusal<'a> = (&'static str, Vec<(&'static str, &'a str)>, &'static str);

fn identities(long: &str) -> Vec<Refusal<'_>> {
    [
        "",
        " Bubble",
        "Bubble ",
        "Bubble  Santi",
        "Bubble\tSanti",
        "Bubble:Santi",
        "Bubble/Santi",
        "Bübble",
        "Bubble`Santi",
    ]
    .into_iter()
    .chain([long])
    .map(|identity| {
        (
            "identity",
            vec![("GROK_BOT_IDENTITY", identity)],
            "concord.issue.comment.operator",
        )
    })
    .collect()
}
