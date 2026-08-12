use super::{emit, explicit};
use crate::args::task::{Command, Dependency};
use concord_core::{Cut, Estate, Finish, Link, Origin, Rehome, Rename, Result, Tune, Weight};
use serde_json::json;

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::List { domain, retired } => {
            let mut tasks = estate.nodes(retired).await?;
            if let Some(domain) = domain {
                tasks.retain(|task| task.domain == domain);
            }
            emit(json!({"tasks": tasks}), output)
        }
        Command::Show { task } => emit(
            json!({
                "current": estate.current(&task).await?,
                "phases": estate.phases(&task).await?,
            }),
            output,
        ),
        Command::Start { domain, name } => {
            emit(json!({"task": estate.start(&domain, &name).await?}), output)
        }
        Command::Change { input } => {
            let patch = super::input::read(&input)?;
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
