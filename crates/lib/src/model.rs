use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Registry {
    pub version: u32,
    #[serde(default)]
    pub task: Vec<Task>,
    #[serde(default)]
    pub repo: Vec<Repo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Task {
    pub name: String,
    #[serde(default)]
    pub repo: Vec<Member>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Member {
    pub name: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Repo {
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

impl Registry {
    pub fn empty() -> Self {
        Self {
            version: 1,
            task: Vec::new(),
            repo: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Error::new(format!(
                "unsupported registry version {}",
                self.version
            )));
        }
        let mut tasks = std::collections::BTreeSet::new();
        for task in &self.task {
            component("task name", &task.name)?;
            if !tasks.insert(&task.name) {
                return Err(Error::new(format!("duplicate task {}", task.name)));
            }
            let mut members = std::collections::BTreeSet::new();
            for member in &task.repo {
                component("member name", &member.name)?;
                if member.source.is_empty() {
                    return Err(Error::new(format!(
                        "member {}/{} has an empty source",
                        task.name, member.name
                    )));
                }
                if let Some(branch) = &member.branch
                    && branch.is_empty()
                {
                    return Err(Error::new(format!(
                        "member {}/{} has an empty branch",
                        task.name, member.name
                    )));
                }
                if !members.insert(&member.name) {
                    return Err(Error::new(format!(
                        "duplicate member {}/{}",
                        task.name, member.name
                    )));
                }
            }
        }
        Ok(())
    }
}

impl Task {
    pub fn new(name: String) -> Self {
        Self {
            name,
            repo: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

impl Member {
    pub fn branch<'a>(&'a self, task: &'a str) -> &'a str {
        self.branch.as_deref().unwrap_or(task)
    }
}

pub fn component(label: &str, value: &str) -> Result<()> {
    if value.is_empty() || value == "." || value == ".." {
        return Err(Error::new(format!("{label} is not a path component")));
    }
    if value.contains('/') || value.contains('\\') || value.contains('\0') {
        return Err(Error::new(format!(
            "{label} is not a single path component"
        )));
    }
    Ok(())
}
