mod artifact;
mod domain;
mod member;
mod task;

pub use task::{
    TASK_BRIEF_LIMIT, TASK_BRIEF_SECTION_BYTES, TaskBriefEntry, TaskBriefLimits, TaskBriefPage,
};

use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub operation: String,
    pub actions: Vec<Action>,
    pub applied: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Action {
    pub verb: String,
    pub target: String,
    pub detail: String,
}

pub struct Add<'a> {
    pub task: &'a str,
    pub name: &'a str,
    pub source: &'a Path,
    pub branch: Option<&'a str>,
    pub orphan: bool,
    pub write: &'a [String],
}

#[derive(Clone, Debug)]
pub struct MigrationClaim {
    pub task: String,
    pub member: String,
    pub write: String,
}

impl Plan {
    fn new(operation: &str, actions: Vec<Action>, applied: bool) -> Self {
        Self {
            operation: operation.to_string(),
            actions,
            applied,
        }
    }

    pub fn single(operation: &str, verb: &str, target: &Path, detail: impl Into<String>) -> Self {
        Self::new(operation, vec![action(verb, target, detail)], false)
    }
}

fn action(verb: &str, target: &Path, detail: impl Into<String>) -> Action {
    Action {
        verb: verb.to_string(),
        target: target.display().to_string(),
        detail: detail.into(),
    }
}
