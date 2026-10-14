pub(super) fn body(prose: &str, agent: &str, session: &str, host: Option<&str>) -> String {
    let value = host
        .map(|host| format!("\"{host}\""))
        .unwrap_or_else(|| "null".to_string());
    let execution = format!("{{\"agent\":\"{agent}\",\"session\":\"{session}\",\"host\":{value}}}");
    let host = host
        .map(|host| format!(" · host `{host}`"))
        .unwrap_or_default();
    let title = match agent {
        "grok-bot" => format!("**🫧 {session}** · Grok Bot"),
        "claude" => "**Claude**".to_string(),
        "codex" => "**Codex**".to_string(),
        "grok" => "**Grok**".to_string(),
        "santi" => "**Santi**".to_string(),
        _ => unreachable!(),
    };
    format!(
        "{prose}\n\n> {title}\n> concord session `{session}`{host}\n\n<!-- concord.issue-comment/v1\n{execution}\n-->"
    )
}
