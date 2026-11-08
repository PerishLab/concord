use crate::activity::{Agent, Operator};
use crate::{Error, Result};
use std::path::Path;

pub(crate) fn search(
    paths: &[&Path],
) -> std::result::Result<std::ffi::OsString, std::env::JoinPathsError> {
    std::env::join_paths(paths)
}

pub(crate) fn tools() -> std::result::Result<radicula_process::Search, String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    radicula_process::Search::new(&root, std::env::var_os("PATH"), std::env::var_os("PATHEXT"))
        .map_err(|error| error.to_string())
}

pub(crate) fn operator() -> Option<Operator> {
    let variables = [
        (Agent::Claude, "CLAUDE_CODE_SESSION_ID"),
        (Agent::Grok, "GROK_SESSION_ID"),
        (Agent::Codex, "CODEX_THREAD_ID"),
        (Agent::GrokBot, "GROK_BOT_IDENTITY"),
    ];
    let mut found = Vec::new();
    for (agent, variable) in variables {
        if let Ok(session) = std::env::var(variable) {
            let operator = Operator { agent, session };
            if operator.valid() {
                found.push(operator);
            }
        }
    }
    let soul = std::env::var_os("SANTI_SOUL_ID");
    let strand = std::env::var_os("SANTI_STRAND_ID");
    if soul.is_some() || strand.is_some() {
        let soul = soul?.into_string().ok()?;
        let strand = strand?.into_string().ok()?;
        if !coordinate(&soul) || !coordinate(&strand) {
            return None;
        }
        let operator = Operator {
            agent: Agent::Santi,
            session: format!("{soul}:{strand}"),
        };
        if !operator.valid() {
            return None;
        }
        found.push(operator);
    }
    match found.len() {
        1 => found.pop(),
        _ => None,
    }
}

fn coordinate(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

pub(crate) fn identity(value: &str) -> bool {
    value.len() <= 128 && value.split(' ').all(coordinate)
}

pub fn session() -> Option<String> {
    operator().map(|operator| operator.session)
}

pub fn host() -> Result<Option<String>> {
    let Some(value) = std::env::var_os("CONCORD_HOST_ID") else {
        return Ok(None);
    };
    let value = value.into_string().map_err(|_| {
        Error::typed(
            "concord.host.encoding",
            "CONCORD_HOST_ID must be valid UTF-8",
        )
    })?;
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/')
        })
    {
        return Err(Error::typed(
            "concord.host.invalid",
            "CONCORD_HOST_ID must be 1..=128 ASCII letters, digits, or -_.:/",
        ));
    }
    Ok(Some(value))
}
