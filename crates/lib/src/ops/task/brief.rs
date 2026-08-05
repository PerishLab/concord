use crate::memory::{Memory, MemoryBrief};
use crate::{Error, Result, Space, Task};
use serde::Serialize;

pub const TASK_BRIEF_LIMIT: usize = 64;
pub const TASK_BRIEF_SECTION_BYTES: usize = 512;

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
pub struct TaskBriefLimits {
    pub tasks: usize,
    pub section_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskBriefEntry {
    pub identity: String,
    pub task: Task,
    pub memory: MemoryBrief,
}

impl Space {
    pub fn task_brief(&self, domain_name: &str, after: Option<&str>) -> Result<TaskBriefPage> {
        let domain = self.domain(domain_name)?;
        let snapshot = domain.read()?;
        let total = snapshot.registry.task.len();
        let start = match after {
            Some(cursor) => snapshot
                .registry
                .task
                .iter()
                .position(|task| task.name == cursor)
                .map(|index| index + 1)
                .ok_or_else(|| {
                    Error::typed(
                        "task.brief_cursor",
                        format!(
                            "task brief cursor does not exist in domain {domain_name}: {cursor}"
                        ),
                    )
                })?,
            None => 0,
        };
        let mut selected = snapshot
            .registry
            .task
            .into_iter()
            .skip(start)
            .take(TASK_BRIEF_LIMIT + 1)
            .collect::<Vec<_>>();
        let next = if selected.len() > TASK_BRIEF_LIMIT {
            selected.truncate(TASK_BRIEF_LIMIT);
            selected.last().map(|task| task.name.clone())
        } else {
            None
        };
        let keys = ["focus".to_string(), "next".to_string()];
        let tasks = selected
            .into_iter()
            .map(|task| {
                let bound = domain.bind_task(task.clone());
                let memory = Memory::new(&bound)
                    .brief_sections(&keys, TASK_BRIEF_SECTION_BYTES)
                    .unwrap_or_else(|error| MemoryBrief::unavailable(&error));
                TaskBriefEntry {
                    identity: bound.identity(),
                    task,
                    memory,
                }
            })
            .collect::<Vec<_>>();
        Ok(TaskBriefPage {
            schema: "concord.task-brief:v1",
            domain: domain_name.to_string(),
            total,
            returned: tasks.len(),
            after: after.map(str::to_string),
            next,
            limits: TaskBriefLimits {
                tasks: TASK_BRIEF_LIMIT,
                section_bytes: TASK_BRIEF_SECTION_BYTES,
            },
            tasks,
        })
    }
}
