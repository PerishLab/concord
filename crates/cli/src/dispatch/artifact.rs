use crate::args::{MemoryCommand, ResourceCommand};
use crate::dispatch::guarded;
use crate::output;
use concord_core::{Memory, Plan, Result, Space};
use serde_json::json;
use std::io::Read;
use std::path::Path;

pub fn memory_command(space: &Space, command: MemoryCommand, json_output: bool) -> Result<()> {
    match command {
        MemoryCommand::Init {
            task,
            file,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            let content = read(&file)?;
            let mut plan = Plan::single("memory.init", "write", &memory.main(), "initial MAIN.md");
            if dry_run {
                return output::mutation(&plan, None, json_output);
            }
            memory.init(&content)?;
            plan.applied = true;
            output::mutation(&plan, None, json_output)
        }
        MemoryCommand::Read { task } => {
            let task = space.resolve(&task)?;
            let read = Memory::new(&task).read()?;
            if json_output {
                output::value(
                    serde_json::to_value(read).expect("memory read should encode"),
                    true,
                );
            } else {
                println!("revision: {}\n{}", read.revision, read.content);
            }
            Ok(())
        }
        MemoryCommand::Write {
            task,
            expect,
            file,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            let content = read(&file)?;
            let plan = Plan::single(
                "memory.write",
                "replace",
                &memory.main(),
                format!("expected revision {expect}"),
            );
            apply_write(plan, &memory, &expect, &content, dry_run, json_output)
        }
        MemoryCommand::Settle {
            task,
            expect,
            phase_file,
            main_file,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            if stdin(&phase_file) && stdin(&main_file) {
                return Err(concord_core::Error::new(
                    "memory settle accepts stdin for only one input",
                ));
            }
            let plan = Plan::single(
                "memory.settle",
                "settle",
                &memory.root(),
                format!("phase plus MAIN.md at expected revision {expect}"),
            );
            let phase = read(&phase_file)?;
            let main = read(&main_file)?;
            apply_settle(plan, &memory, &expect, &phase, &main, dry_run, json_output)
        }
        MemoryCommand::Remove { task, apply } => guarded(
            space.memory_remove(&task, false)?,
            apply,
            json_output,
            || space.memory_remove(&task, true),
        ),
    }
}

fn apply_write(
    mut plan: Plan,
    memory: &Memory<'_>,
    expect: &str,
    content: &str,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    if dry_run {
        return output::mutation(&plan, None, json_output);
    }
    let read = memory.write(expect, content)?;
    plan.applied = true;
    output::mutation(
        &plan,
        Some(json!({"path": read.path, "revision": read.revision})),
        json_output,
    )
}

fn apply_settle(
    mut plan: Plan,
    memory: &Memory<'_>,
    expect: &str,
    phase: &str,
    main: &str,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    if dry_run {
        return output::mutation(&plan, None, json_output);
    }
    let (read, phase) = memory.settle(expect, phase, main)?;
    plan.applied = true;
    output::mutation(
        &plan,
        Some(json!({"phase": phase.display().to_string(), "revision": read.revision})),
        json_output,
    )
}

pub fn resource_command(space: &Space, command: ResourceCommand, json_output: bool) -> Result<()> {
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

fn read(path: &Path) -> Result<String> {
    if stdin(path) {
        let mut content = String::new();
        std::io::stdin()
            .read_to_string(&mut content)
            .map_err(|error| concord_core::Error::new(format!("cannot read stdin: {error}")))?;
        return Ok(content);
    }
    std::fs::read_to_string(path).map_err(|error| {
        concord_core::Error::new(format!("cannot read input {}: {error}", path.display()))
    })
}

fn stdin(path: &Path) -> bool {
    path == Path::new("-")
}
