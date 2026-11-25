use super::*;

#[test]
fn documents() {
    for body in [
        "{}",
        " \n[{}]\n",
        "\"Decision\"",
        "42",
        "1e999",
        "true",
        "null",
    ] {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let mock = guard();
        let output = invoke(
            fixture.path(),
            Run {
                server: &mock.server,
                body,
                environment: &[("CODEX_THREAD_ID", "thread")],
                timeout: 10,
            },
        );
        assert!(!output.status.success(), "{body}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("concord.issue.comment.json"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(mock.hits(), 0);
    }
}

#[test]
fn prose() {
    for body in [
        "Decision: accepted.",
        "```json\n{}\n```",
        "Example: {\"ok\": true}",
        "{} followed by prose",
    ] {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let expected = render::body(body, "codex", "thread", None);
        let mock = provider("OPEN", &expected, Answer::Exact);
        success(invoke(
            fixture.path(),
            Run {
                server: &mock.server,
                body,
                environment: &[("CODEX_THREAD_ID", "thread")],
                timeout: 10,
            },
        ));
        request(&mock, &expected);
    }
}

#[test]
fn validation() {
    let long = "a".repeat(129);
    let over = "a".repeat(513);
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
            "agy",
            vec![
                ("ANTIGRAVITY_CONVERSATION_ID", "agy-session"),
                ("CODEX_THREAD_ID", "thread"),
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
    .chain(sessions(&over))
    {
        let fixture = tempfile::tempdir().expect("fixture");
        bootstrap(fixture.path());
        let mock = guard();
        let output = invoke(
            fixture.path(),
            Run {
                server: &mock.server,
                body: "Declaration",
                environment: &environment,
                timeout: 10,
            },
        );
        assert!(!output.status.success(), "{name}");
        assert!(String::from_utf8_lossy(&output.stderr).contains(code));
        assert_eq!(mock.hits(), 0);
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

fn sessions(over: &str) -> Vec<Refusal<'_>> {
    ["", "bad session", "agy!invalid", "agy`session", "ägy"]
        .into_iter()
        .chain([over])
        .map(|session| {
            (
                "session",
                vec![("ANTIGRAVITY_CONVERSATION_ID", session)],
                "concord.issue.comment.operator",
            )
        })
        .collect()
}
