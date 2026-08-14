use super::activity;
use super::{emit, explicit};
use crate::args::task::{Command, Dependency};
use concord_core::{
    Cut, Estate, Finish, Link, Origin, Patch, Rehome, Rename, Result, Tune, Weight,
};
use serde_json::json;

pub async fn run(
    estate: &Estate,
    command: Command,
    activity: &activity::Run,
    output: bool,
) -> Result<()> {
    let operation = command.name();
    match command {
        Command::List { domain, retired } => {
            let mut tasks = estate.nodes(retired).await?;
            if let Some(domain) = domain {
                tasks.retain(|task| task.domain == domain);
            }
            emit(json!({"tasks": tasks}), output)
        }
        Command::Brief { domain, after } => brief(estate, &domain, after.as_deref(), output).await,
        Command::Show { task } => emit(
            json!({
                "current": estate.current(&task).await?,
                "phases": estate.phases(&task).await?,
            }),
            output,
        ),
        Command::Start { domain, name } => {
            let task = estate.start(&domain, &name).await?;
            activity.touch(estate, &task.identity(), operation).await;
            emit(json!({"task": task}), output)
        }
        Command::Change { input } => {
            let patch: Patch = super::input::read(&input, Patch::SHAPE)?;
            activity.touch(estate, &patch.task, operation).await;
            emit(json!({"current": estate.change(&patch).await?}), output)
        }
        Command::Rename {
            task,
            name,
            revision,
        } => emit(
            json!({"task": estate.rename(&Rename { task, name, revision }).await?}),
            output,
        ),
        Command::Rehome {
            task,
            domain,
            revision,
        } => emit(
            json!({"task": estate.rehome(&Rehome { task, domain, revision }).await?}),
            output,
        ),
        Command::Dependency { command } => dependency(estate, command, output).await,
        Command::Finish {
            task,
            revision,
            graph,
            reason,
            apply,
        } => {
            explicit(apply, "task finish")?;
            let finish = Finish {
                task,
                revision,
                graph,
                reason,
            };
            emit(json!({"task": estate.finish(&finish).await?}), output)
        }
    }
}

async fn brief(estate: &Estate, domain: &str, after: Option<&str>, output: bool) -> Result<()> {
    let brief = estate.task_brief(domain, after).await?;
    if output {
        return emit(json!({"brief": brief}), true);
    }
    super::output::task_brief(&brief);
    Ok(())
}

async fn dependency(estate: &Estate, command: Dependency, output: bool) -> Result<()> {
    match command {
        Dependency::Add {
            source,
            target,
            weight,
            revision,
            create,
        } => {
            let link = Link {
                source,
                target,
                weight: Weight::parse(&weight)?,
                origin: Origin::Declared,
                revision,
            };
            emit(
                json!({"revision": estate.declare(&link, create).await?}),
                output,
            )
        }
        Dependency::Set {
            source,
            target,
            weight,
            revision,
        } => {
            let tune = Tune {
                source,
                target,
                weight: Weight::parse(&weight)?,
                revision,
            };
            emit(json!({"revision": estate.weigh(&tune).await?}), output)
        }
        Dependency::Remove {
            source,
            target,
            revision,
            reason,
            apply,
        } => {
            explicit(apply, "dependency remove")?;
            let cut = Cut {
                source,
                target,
                reason,
                revision,
            };
            emit(json!({"revision": estate.detach(&cut).await?}), output)
        }
        Dependency::List { task, direction } => {
            let flow = super::graph::flow(&direction)?;
            let graph = estate.graph(true).await?;
            let node = estate.node(&task).await?;
            let edges = super::graph::links(&graph, Some(node.key), flow);
            emit(
                json!({"revision": graph.revision, "task": node, "dependencies": edges}),
                output,
            )
        }
    }
}
