pub(super) fn body(prose: &str, agent: &str, session: &str, host: Option<&str>) -> String {
    let value = host
        .map(|host| format!("\"{host}\""))
        .unwrap_or_else(|| "null".to_string());
    let execution = format!("{{\"agent\":\"{agent}\",\"session\":\"{session}\",\"host\":{value}}}");
    let host = host
        .map(|host| format!(" · host `{host}`"))
        .unwrap_or_default();
    format!(
        "{prose}\n\n---\nConcord: `{agent}` session `{session}`{host}\n\n<!-- concord.issue-comment/v1\n{execution}\n-->"
    )
}
