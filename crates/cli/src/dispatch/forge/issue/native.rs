use crate::runtime::github;
use concord_core::{Error, Result};
use runseal::github::{Comment, Outcome};

pub(super) fn comment(subject: &str, body: &str, timeout: u64) -> Result<Comment> {
    let client = github::client(timeout)
        .map_err(|error| Error::typed("concord.issue.comment.command", error))?;
    github::call("issue.comment", || client.comment(subject, body))
        .map(|held| held.value)
        .map_err(|error| {
            let code = match error.outcome {
                Outcome::Unsent => "concord.issue.comment.command",
                Outcome::Refused => "concord.issue.comment.provider",
                _ => "concord.issue.comment.indeterminate",
            };
            Error::typed(code, github::describe(error))
        })
}
