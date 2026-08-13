use crate::activity::{Agent, Operator};
use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Root {
    path: PathBuf,
}

impl Root {
    pub fn new(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(Error::new("domain_space_root must be absolute"));
        }
        let path = path.canonicalize().map_err(|error| {
            Error::new(format!(
                "cannot resolve domain_space_root {}: {error}",
                path.display()
            ))
        })?;
        if !path.is_dir() {
            return Err(Error::new(format!(
                "domain_space_root is not a directory: {}",
                path.display()
            )));
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn home() -> Result<PathBuf> {
    plumb::config::home().ok_or_else(|| Error::new("HOME is required to expand repository source"))
}

pub(crate) fn operator() -> Option<Operator> {
    let variables = [
        (Agent::Claude, "CLAUDE_CODE_SESSION_ID"),
        (Agent::Grok, "GROK_SESSION_ID"),
        (Agent::Codex, "CODEX_THREAD_ID"),
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
    match found.len() {
        1 => found.pop(),
        _ => None,
    }
}
