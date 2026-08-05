use crate::args::{TaskCommand, TaskTodoCommand};
use crate::output;
use concord_core::{Result, Space};
use serde_json::json;

use super::{create, guarded};

pub(crate) fn command(space: &Space, command: TaskCommand, json_output: bool) -> Result<()> {
    match command {
        TaskCommand::List { domain } => {
            let values = tasks(space, domain.as_deref())?;
            output::value(serde_json::Value::Array(values), json_output);
            Ok(())
        }
        TaskCommand::Brief { domain, after } => {
            let brief = space.task_brief(&domain, after.as_deref())?;
            output::task_brief(&brief, json_output)
        }
        TaskCommand::Show { task } => {
            let task = space.resolve(&task)?;
            output::value(
                json!({
                    "identity": task.identity(),
                    "path": task.path().display().to_string(),
                    "task": task.task()
                }),
                json_output,
            );
            Ok(())
        }
        TaskCommand::Start { task, dry_run } => create(
            space.task_start(&task, false)?,
            dry_run,
            json_output,
            || space.task_start(&task, true),
        ),
        TaskCommand::Todo { command } => todo(space, command, json_output),
        TaskCommand::Rename { task, name, apply } => guarded(
            space.task_rename(&task, &name, false)?,
            apply,
            json_output,
            || space.task_rename(&task, &name, true),
        ),
        TaskCommand::Rehome {
            task,
            domain,
            apply,
        } => guarded(
            space.task_rehome(&task, &domain, false)?,
            apply,
            json_output,
            || space.task_rehome(&task, &domain, true),
        ),
        TaskCommand::Finish { task, apply } => {
            guarded(space.task_finish(&task, false)?, apply, json_output, || {
                space.task_finish(&task, true)
            })
        }
    }
}

fn todo(space: &Space, command: TaskTodoCommand, json_output: bool) -> Result<()> {
    match command {
        TaskTodoCommand::Add {
            task,
            target,
            dry_run,
        } => create(
            space.task_todo_add(&task, &target, false)?,
            dry_run,
            json_output,
            || space.task_todo_add(&task, &target, true),
        ),
        TaskTodoCommand::Remove {
            task,
            target,
            apply,
        } => guarded(
            space.task_todo_remove(&task, &target, false)?,
            apply,
            json_output,
            || space.task_todo_remove(&task, &target, true),
        ),
    }
}

fn tasks(space: &Space, selected: Option<&str>) -> Result<Vec<serde_json::Value>> {
    let domains = match selected {
        Some(name) => vec![space.domain(name)?],
        None => space.domains()?,
    };
    let mut values = Vec::new();
    for domain in domains {
        let name = domain.name().to_string();
        values.extend(domain.registry()?.task.into_iter().map(|task| {
            json!({
                "identity": format!("{}/{}", name, task.name),
                "task": task
            })
        }));
    }
    Ok(values)
}
