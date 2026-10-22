use std::process::ExitStatus;

pub(super) fn message(status: ExitStatus, stderr: &[u8]) -> String {
    let reported = reported(stderr);
    format!(
        "GitHub Issue projection was refused by the provider (api graphql; {status}; {reported}). Provider stderr is withheld; read concord cookbook concord.issue.projection.provider before resuming delivery."
    )
}

fn reported(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    if let Some(status) = http(&text) {
        return format!("reported HTTP status {status}");
    }
    for (needle, label) in [
        ("TLS handshake timeout", "TLS handshake timeout"),
        ("no such host", "DNS lookup failure"),
        ("connection refused", "connection refusal"),
        ("connection reset by peer", "connection reset"),
        ("context deadline exceeded", "request deadline exceeded"),
        ("unexpected EOF", "unexpected EOF"),
    ] {
        if text.contains(needle) {
            return format!("reported transport diagnostic: {label}");
        }
    }
    "no recognized provider diagnostic; cause unknown".into()
}

fn http(text: &str) -> Option<u16> {
    text.split("HTTP ").skip(1).find_map(|tail| {
        let code = tail.get(..3)?.parse::<u16>().ok()?;
        let boundary = tail
            .as_bytes()
            .get(3)
            .is_none_or(|byte| !byte.is_ascii_alphanumeric());
        ((100..=599).contains(&code) && boundary).then_some(code)
    })
}
