use crate::args::{MemoryCommand, MemoryPhaseCommand};
use crate::dispatch::guarded;
use crate::dispatch::input::{Input, is_stdin};
use crate::output;
use concord_core::{Error, MAX_MAIN_BYTES, MAX_PHASE_BYTES, Memory, Plan, Result, Space};
use serde_json::json;

#[locus::trace(with = concord_core::observation::view())]
pub fn memory_command(space: &Space, command: MemoryCommand, json_output: bool) -> Result<()> {
    match command {
        MemoryCommand::Init {
            task,
            file,
            keep_file,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            let input = Input::load(&file, MAX_MAIN_BYTES, &memory.root())?;
            let mut plan = Plan::single("memory.init", "write", &memory.main(), "initial MAIN.md");
            if dry_run {
                return output::mutation(&plan, None, json_output);
            }
            memory.init(input.content())?;
            let read = memory.read()?;
            plan.applied = true;
            let result = json!({
                "path": read.path,
                "revision": read.revision,
                "changed": true,
            });
            consume(&[&input], keep_file, &result)?;
            output::mutation(&plan, Some(result), json_output)
        }
        MemoryCommand::Read { task, section } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            let projected = !section.is_empty();
            let read = if projected {
                memory.read_sections(&section)?
            } else {
                memory.read()?
            };
            if json_output {
                output::value(
                    serde_json::to_value(read).expect("memory read should encode"),
                    true,
                );
            } else if projected {
                print!("{}", read.content);
            } else {
                println!("revision: {}\n{}", read.revision, read.content);
            }
            Ok(())
        }
        MemoryCommand::Patch {
            task,
            expect,
            file,
            keep_file,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            let input = Input::load(&file, MAX_MAIN_BYTES, &memory.root())?;
            let mut plan = Plan::single(
                "memory.patch",
                "patch",
                &memory.main(),
                "sparse concord-memory:v1 section update",
            );
            if dry_run {
                return output::mutation(&plan, None, json_output);
            }
            let change = memory.patch(expect.as_deref(), input.content())?;
            plan.applied = true;
            let result = serde_json::to_value(change).expect("memory change should encode");
            consume(&[&input], keep_file, &result)?;
            output::mutation(&plan, Some(result), json_output)
        }
        MemoryCommand::Write {
            task,
            expect,
            file,
            keep_file,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            let input = Input::load(&file, MAX_MAIN_BYTES, &memory.root())?;
            let mut plan = Plan::single(
                "memory.write",
                "replace",
                &memory.main(),
                format!("expected revision {expect}"),
            );
            if dry_run {
                return output::mutation(&plan, None, json_output);
            }
            let change = memory.write(&expect, input.content())?;
            plan.applied = true;
            let result = serde_json::to_value(change).expect("memory change should encode");
            consume(&[&input], keep_file, &result)?;
            output::mutation(&plan, Some(result), json_output)
        }
        MemoryCommand::Settle {
            task,
            expect,
            phase_file,
            main_file,
            keep_files,
            dry_run,
        } => {
            let task = space.resolve(&task)?;
            let memory = Memory::new(&task);
            if is_stdin(&phase_file) && is_stdin(&main_file) {
                return Err(Error::typed(
                    "memory.input_ambiguous",
                    "memory settle accepts stdin for only one input",
                ));
            }
            let phase = Input::load(&phase_file, MAX_PHASE_BYTES, &memory.root())?;
            let main = Input::load(&main_file, MAX_MAIN_BYTES, &memory.root())?;
            if phase.same_file(&main) {
                return Err(Error::typed(
                    "memory.input_duplicate",
                    "memory settle requires distinct explicit input paths",
                ));
            }
            let mut plan = Plan::single(
                "memory.settle",
                "settle",
                &memory.root(),
                format!("phase plus MAIN.md at expected revision {expect}"),
            );
            if dry_run {
                return output::mutation(&plan, None, json_output);
            }
            let (change, phase_path) = memory.settle(&expect, phase.content(), main.content())?;
            plan.applied = true;
            let result = json!({
                "phase": phase_path.display().to_string(),
                "path": change.path,
                "revision": change.revision,
                "changed": change.changed,
            });
            consume(&[&phase, &main], keep_files, &result)?;
            output::mutation(&plan, Some(result), json_output)
        }
        MemoryCommand::Phase { command } => phase_command(space, command, json_output),
        MemoryCommand::Remove { task, apply } => guarded(
            space.memory_remove(&task, false)?,
            apply,
            json_output,
            || space.memory_remove(&task, true),
        ),
    }
}

fn phase_command(space: &Space, command: MemoryPhaseCommand, json_output: bool) -> Result<()> {
    match command {
        MemoryPhaseCommand::List { task } => {
            let task = space.resolve(&task)?;
            let phases = Memory::new(&task).phases()?;
            output::value(
                serde_json::to_value(phases).expect("phase list should encode"),
                json_output,
            );
            Ok(())
        }
        MemoryPhaseCommand::Read { task, number } => {
            let task = space.resolve(&task)?;
            let phase = Memory::new(&task).phase(number)?;
            if json_output {
                output::value(
                    serde_json::to_value(phase).expect("phase read should encode"),
                    true,
                );
            } else {
                println!("revision: {}\n{}", phase.revision, phase.content);
            }
            Ok(())
        }
    }
}

#[locus::trace(with = concord_core::observation::view())]
fn consume(inputs: &[&Input], keep: bool, result: &serde_json::Value) -> Result<()> {
    if keep {
        return Ok(());
    }
    for input in inputs {
        if let Err(error) = input.verify_cleanup() {
            return Err(cleanup_error(error, input, result));
        }
    }
    for input in inputs {
        if let Err(error) = input.remove() {
            return Err(cleanup_error(error, input, result));
        }
    }
    Ok(())
}

fn cleanup_error(error: Error, input: &Input, result: &serde_json::Value) -> Error {
    let mut details = result.as_object().cloned().unwrap_or_default();
    details.insert("applied".to_string(), json!(true));
    details.insert("input_path".to_string(), json!(input.path()));
    Error::typed(
        "memory.cleanup_after_apply",
        format!("{error}; memory mutation remains applied and input was retained"),
    )
    .with_details(serde_json::Value::Object(details))
}
