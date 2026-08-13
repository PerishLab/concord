use super::{Estate, Fact, Life, Node, Role, World};
use crate::{Error, Result};
use serde::Serialize;
use std::collections::BTreeMap;

pub const TASK_BRIEF_LIMIT: usize = 64;
pub const TASK_BRIEF_ROLE_BYTES: usize = 512;

const ROLES: [(Role, &str); 4] = [
    (Role::Goal, "goal"),
    (Role::Focus, "focus"),
    (Role::Question, "question"),
    (Role::Next, "next"),
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TaskBriefPage {
    pub schema: &'static str,
    pub domain: String,
    pub total: usize,
    pub returned: usize,
    pub after: Option<String>,
    pub next: Option<String>,
    pub limits: TaskBriefLimits,
    pub tasks: Vec<TaskBriefEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TaskBriefLimits {
    pub tasks: usize,
    pub role_bytes: usize,
    pub roles: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TaskBriefEntry {
    pub identity: String,
    pub task: Node,
    pub roles: BTreeMap<&'static str, RoleBrief>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RoleBrief {
    pub total: usize,
    pub returned: usize,
    pub bytes: usize,
    pub truncated: bool,
    pub facts: Vec<FactBrief>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FactBrief {
    pub key: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<i64>,
    pub body: TextPreview,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TextPreview {
    pub text: String,
    pub bytes: usize,
    pub truncated: bool,
}

impl Estate {
    pub async fn task_brief(&self, domain: &str, after: Option<&str>) -> Result<TaskBriefPage> {
        let world = World::load(self).await?;
        if world.domain(domain).is_none() {
            return Err(Error::typed(
                "concord.domain.absent",
                format!("unknown managed domain {domain}"),
            ));
        }
        let tasks = active(&world, domain);
        let total = tasks.len();
        let start = cursor(&tasks, after, domain)?;
        let mut selected = tasks
            .iter()
            .skip(start)
            .take(TASK_BRIEF_LIMIT + 1)
            .cloned()
            .collect::<Vec<_>>();
        let next = if selected.len() > TASK_BRIEF_LIMIT {
            selected.truncate(TASK_BRIEF_LIMIT);
            selected.last().map(|task| task.name.clone())
        } else {
            None
        };
        let mut entries = Vec::with_capacity(selected.len());
        for task in selected {
            let facts = self.facts(task.key).await?;
            entries.push(TaskBriefEntry {
                identity: task.identity(),
                roles: roles(&facts),
                task,
            });
        }
        let current = active(&World::load(self).await?, domain);
        if !stable(&tasks, &current, &entries) {
            return Err(Error::typed(
                "concord.task.brief_stale",
                format!("Task brief changed while reading Domain {domain}"),
            ));
        }
        Ok(TaskBriefPage {
            schema: "concord.task-brief:v1",
            domain: domain.to_string(),
            total,
            returned: entries.len(),
            after: after.map(str::to_string),
            next,
            limits: TaskBriefLimits {
                tasks: TASK_BRIEF_LIMIT,
                role_bytes: TASK_BRIEF_ROLE_BYTES,
                roles: ROLES.iter().map(|(_, name)| *name).collect(),
            },
            tasks: entries,
        })
    }
}

fn active(world: &World, domain: &str) -> Vec<Node> {
    world
        .nodes
        .iter()
        .filter(|task| task.domain == domain && task.life == Life::Active)
        .cloned()
        .collect()
}

fn stable(before: &[Node], after: &[Node], entries: &[TaskBriefEntry]) -> bool {
    before.len() == after.len()
        && before.iter().zip(after).all(|(left, right)| {
            left.key == right.key && left.domain == right.domain && left.name == right.name
        })
        && entries.iter().all(|entry| {
            after
                .iter()
                .find(|task| task.key == entry.task.key)
                .is_some_and(|task| task.revision == entry.task.revision)
        })
}

fn cursor(tasks: &[Node], after: Option<&str>, domain: &str) -> Result<usize> {
    let Some(after) = after else {
        return Ok(0);
    };
    tasks
        .iter()
        .position(|task| task.name == after)
        .map(|index| index + 1)
        .ok_or_else(|| {
            Error::typed(
                "concord.task.brief_cursor",
                format!("Task brief cursor does not exist in Domain {domain}: {after}"),
            )
        })
}

fn roles(facts: &[Fact]) -> BTreeMap<&'static str, RoleBrief> {
    ROLES
        .iter()
        .filter_map(|(role, name)| brief(facts, *role).map(|brief| (*name, brief)))
        .collect()
}

fn brief(facts: &[Fact], role: Role) -> Option<RoleBrief> {
    let selected = facts
        .iter()
        .filter(|fact| fact.role == role)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return None;
    }
    let bytes = selected.iter().map(|fact| fact.body.len()).sum();
    let mut remaining = TASK_BRIEF_ROLE_BYTES;
    let mut projected = Vec::new();
    for fact in &selected {
        if remaining == 0 {
            break;
        }
        let body = preview(&fact.body, remaining);
        remaining -= body.text.len();
        projected.push(FactBrief {
            key: fact.key.expect("estate Fact has a key"),
            rank: fact.rank,
            body,
        });
    }
    Some(RoleBrief {
        total: selected.len(),
        returned: projected.len(),
        bytes,
        truncated: bytes > TASK_BRIEF_ROLE_BYTES,
        facts: projected,
    })
}

fn preview(text: &str, max: usize) -> TextPreview {
    let bytes = text.len();
    if bytes <= max {
        return TextPreview {
            text: text.to_string(),
            bytes,
            truncated: false,
        };
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    TextPreview {
        text: text[..end].to_string(),
        bytes,
        truncated: true,
    }
}
