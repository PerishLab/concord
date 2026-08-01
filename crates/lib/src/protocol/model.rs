use crate::{Error, Result, claim};
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub todo: Vec<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary: Option<BoundaryProof>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BoundaryProof {
    pub schema: String,
    pub plumb: String,
    pub base: String,
    pub head: String,
    pub claim: String,
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
            version: 3,
            task: Vec::new(),
            repo: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !matches!(self.version, 1..=3) {
            return Err(Error::new(format!(
                "unsupported registry version {}",
                self.version
            )));
        }
        let mut tasks = std::collections::BTreeSet::new();
        for task in &self.task {
            component("task name", &task.name)?;
            if !tasks.insert(task.name.as_str()) {
                return Err(Error::new(format!("duplicate task {}", task.name)));
            }
        }
        for task in &self.task {
            task.validate(self.version, &tasks)?;
        }
        Ok(())
    }
}

impl Task {
    pub fn new(name: String) -> Self {
        Self {
            name,
            todo: Vec::new(),
            repo: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    fn validate(&self, version: u32, tasks: &std::collections::BTreeSet<&str>) -> Result<()> {
        self.todos(version, tasks)?;
        let mut members = std::collections::BTreeSet::new();
        for member in &self.repo {
            member.validate(&self.name, version, &mut members)?;
        }
        Ok(())
    }

    fn todos(&self, version: u32, tasks: &std::collections::BTreeSet<&str>) -> Result<()> {
        if version < 3 {
            if self.todo.is_empty() {
                return Ok(());
            }
            return Err(Error::new(format!(
                "registry version {version} task {} contains version 3 todo state",
                self.name
            )));
        }
        let mut previous = None;
        for target in &self.todo {
            component("todo task name", target)?;
            if target == &self.name {
                return Err(Error::new(format!(
                    "task {} cannot reference itself as a todo",
                    self.name
                )));
            }
            if previous.is_some_and(|held: &String| held >= target) {
                return Err(Error::new(format!(
                    "task {} todo entries are not normalized",
                    self.name
                )));
            }
            if !tasks.contains(target.as_str()) {
                return Err(Error::new(format!(
                    "task {} todo target does not exist: {target}",
                    self.name
                )));
            }
            previous = Some(target);
        }
        Ok(())
    }
}

impl Member {
    pub fn branch<'a>(&'a self, task: &'a str) -> &'a str {
        self.branch.as_deref().unwrap_or(task)
    }

    fn validate<'a>(
        &'a self,
        task: &str,
        version: u32,
        members: &mut std::collections::BTreeSet<&'a String>,
    ) -> Result<()> {
        component("member name", &self.name)?;
        if self.source.is_empty() {
            return Err(Error::new(format!(
                "member {task}/{} has an empty source",
                self.name
            )));
        }
        if self.branch.as_ref().is_some_and(String::is_empty) {
            return Err(Error::new(format!(
                "member {task}/{} has an empty branch",
                self.name
            )));
        }
        if !members.insert(&self.name) {
            return Err(Error::new(format!("duplicate member {task}/{}", self.name)));
        }
        self.version(task, version)
    }

    fn version(&self, task: &str, version: u32) -> Result<()> {
        if version == 1 {
            if self.write.is_empty() && self.boundary.is_none() {
                return Ok(());
            }
            return Err(Error::new(format!(
                "version 1 member {task}/{} contains version 2 boundary state",
                self.name
            )));
        }
        let normalized = claim::normalize(&self.write)?;
        if normalized != self.write {
            return Err(Error::new(format!(
                "member {task}/{} write claims are not normalized",
                self.name
            )));
        }
        if let Some(proof) = &self.boundary {
            proof.validate(task, &self.name)?;
        }
        Ok(())
    }
}

impl BoundaryProof {
    fn validate(&self, task: &str, member: &str) -> Result<()> {
        if self.schema.is_empty() || self.plumb.is_empty() {
            return Err(Error::new(format!(
                "member {task}/{member} boundary proof has empty protocol identity"
            )));
        }
        if !oid(&self.base) || !oid(&self.head) || !hex(&self.claim, 64) {
            return Err(Error::new(format!(
                "member {task}/{member} boundary proof is malformed"
            )));
        }
        Ok(())
    }
}

fn oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && hex(value, value.len())
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
