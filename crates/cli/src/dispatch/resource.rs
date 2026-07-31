use crate::args::ResourceCommand;
use crate::dispatch::guarded;
use crate::output;
use concord_core::{Memory, Plan, Result, Space};
use serde_json::json;

pub fn command(space: &Space, command: ResourceCommand, json_output: bool) -> Result<()> {
    match command {
        ResourceCommand::List { task } => {
            let task = space.resolve(&task)?;
            let values = Memory::new(&task)
                .resources()?
                .into_iter()
                .map(|path| json!(path.display().to_string()))
                .collect();
            output::value(serde_json::Value::Array(values), json_output);
            Ok(())
        }
        ResourceCommand::Show { task, name } => {
            let task = space.resolve(&task)?;
            let path = Memory::new(&task).resource(&name)?;
            output::value(json!(path.display().to_string()), json_output);
            Ok(())
        }
        ResourceCommand::Allocate {
            task,
            name,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            let target = memory.root().join("resources").join(&name);
            let mut plan = Plan::single(
                "resource.allocate",
                "create",
                &target,
                "private resource seat",
            );
            if dry_run {
                return output::mutation(&plan, None, json_output);
            }
            let path = memory.allocate(&name)?;
            plan.applied = true;
            output::mutation(&plan, Some(json!(path.display().to_string())), json_output)
        }
        ResourceCommand::Import {
            task,
            name,
            source,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            let target = memory.root().join("resources").join(&name);
            let preflight = memory.preflight_import(&name, &source)?;
            let mut plan = Plan::single(
                "resource.import",
                "import",
                &target,
                format!(
                    "private copy of {}; {} entries, {} required bytes, {} available bytes, {} reserved bytes",
                    preflight.source,
                    preflight.entries,
                    preflight.required_bytes,
                    preflight.available_bytes,
                    preflight.reserve_bytes
                ),
            );
            if dry_run {
                return output::mutation(&plan, None, json_output);
            }
            let path = memory.import(&name, &source)?;
            plan.applied = true;
            output::mutation(&plan, Some(json!(path.display().to_string())), json_output)
        }
        ResourceCommand::Remove { task, name, apply } => guarded(
            space.resource_remove(&task, &name, false)?,
            apply,
            json_output,
            || space.resource_remove(&task, &name, true),
        ),
    }
}
