use super::emit;
use crate::args::graph::Command;
use concord_core::{Error, Estate, Flow, Graph, Node, Result, Weight};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::Adjacency {
            domain,
            minimum,
            retired,
        } => {
            let graph = select(estate.graph(retired).await?, domain.as_deref(), &minimum)?;
            let adjacency = graph
                .adjacency()
                .into_iter()
                .map(|(key, pair)| adjacent(&graph, key, pair))
                .collect::<Vec<_>>();
            emit(
                json!({"revision": graph.revision, "adjacency": adjacency}),
                output,
            )
        }
        Command::Neighbors {
            task,
            direction,
            depth,
            minimum,
        } => {
            let graph = estate.graph(false).await?;
            let node = estate.node(&task).await?;
            let keys =
                graph.neighbors(node.key, flow(&direction)?, depth, Weight::parse(&minimum)?);
            emit(
                json!({"revision": graph.revision, "task": node, "neighbors": names(&graph, keys)}),
                output,
            )
        }
        Command::Degree { task, minimum } => {
            let graph = estate.graph(false).await?;
            let node = estate.node(&task).await?;
            emit(
                json!({
                    "revision": graph.revision,
                    "task": node,
                    "degree": graph.degree(node.key, Weight::parse(&minimum)?),
                }),
                output,
            )
        }
        Command::Reach {
            task,
            direction,
            minimum,
        } => {
            let graph = estate.graph(false).await?;
            let node = estate.node(&task).await?;
            let keys = graph.reach(node.key, flow(&direction)?, Weight::parse(&minimum)?);
            emit(
                json!({"revision": graph.revision, "task": node, "reach": names(&graph, keys)}),
                output,
            )
        }
        Command::Path {
            source,
            target,
            minimum,
            all,
            shortest: _,
        } => {
            let graph = estate.graph(false).await?;
            let source = estate.node(&source).await?;
            let target = estate.node(&target).await?;
            if all {
                let paths = graph
                    .paths(source.key, target.key, Weight::parse(&minimum)?, 1024)?
                    .into_iter()
                    .map(|keys| names(&graph, keys))
                    .collect::<Vec<_>>();
                return emit(routes(graph.revision, &source, &target, paths), output);
            }
            let path = graph
                .path(source.key, target.key, Weight::parse(&minimum)?)
                .map(|keys| names(&graph, keys));
            emit(
                json!({"revision": graph.revision, "source": source, "target": target, "path": path}),
                output,
            )
        }
        Command::Cycles { domain, retired } => {
            let graph = select(estate.graph(retired).await?, domain.as_deref(), "unknown")?;
            let cycles = graph
                .cycles()
                .into_iter()
                .map(|keys| names(&graph, keys))
                .collect::<Vec<_>>();
            emit(
                json!({"revision": graph.revision, "cycles": cycles}),
                output,
            )
        }
        Command::Scc { domain, retired } => {
            let graph = select(estate.graph(retired).await?, domain.as_deref(), "unknown")?;
            let groups = graph
                .scc()
                .into_iter()
                .map(|keys| names(&graph, keys))
                .collect::<Vec<_>>();
            emit(
                json!({"revision": graph.revision, "components": groups}),
                output,
            )
        }
        Command::Export {
            domain,
            minimum,
            retired,
            from,
            depth,
        } => {
            let mut graph = select(estate.graph(retired).await?, domain.as_deref(), &minimum)?;
            if let Some(from) = from {
                let node = estate.node(&from).await?;
                graph = rooted(graph, node.key, depth.unwrap_or(1))?;
            }
            let edges = links(&graph, None, Flow::Both);
            emit(
                json!({"revision": graph.revision, "nodes": graph.nodes, "dependencies": edges}),
                output,
            )
        }
    }
}

fn routes(revision: i64, source: &Node, target: &Node, paths: Vec<Vec<String>>) -> Value {
    json!({"revision": revision, "source": source, "target": target, "paths": paths})
}

fn rooted(mut graph: Graph, root: i64, depth: usize) -> Result<Graph> {
    if !graph.nodes.iter().any(|node| node.key == root) {
        return Err(Error::typed(
            "concord.graph.root",
            "graph root is outside the selected export",
        ));
    }
    let mut keys = graph
        .neighbors(root, Flow::Out, depth, Weight::Unknown)
        .into_iter()
        .collect::<BTreeSet<_>>();
    keys.insert(root);
    graph.nodes.retain(|node| keys.contains(&node.key));
    graph
        .edges
        .retain(|edge| keys.contains(&edge.source) && keys.contains(&edge.target));
    Ok(graph)
}

fn adjacent(held: &Graph, key: i64, pair: (Vec<i64>, Vec<i64>)) -> Value {
    json!({
        "task": identity(held, key),
        "out": names(held, pair.0),
        "in": names(held, pair.1),
    })
}

pub fn flow(value: &str) -> Result<Flow> {
    match value {
        "out" => Ok(Flow::Out),
        "in" => Ok(Flow::In),
        "both" => Ok(Flow::Both),
        _ => Err(Error::typed(
            "concord.graph.direction",
            format!("invalid graph direction {value}"),
        )),
    }
}

pub fn links(network: &Graph, task: Option<i64>, flow: Flow) -> Vec<Value> {
    network
        .edges
        .iter()
        .filter(|edge| match (task, flow) {
            (None, _) => true,
            (Some(key), Flow::Out) => edge.source == key,
            (Some(key), Flow::In) => edge.target == key,
            (Some(key), Flow::Both) => edge.source == key || edge.target == key,
        })
        .map(|edge| {
            json!({
                "key": edge.key,
                "source": identity(network, edge.source),
                "target": identity(network, edge.target),
                "weight": edge.weight,
                "origin": edge.origin,
            })
        })
        .collect()
}

fn select(mut graph: Graph, domain: Option<&str>, minimum: &str) -> Result<Graph> {
    let minimum = Weight::parse(minimum)?;
    if let Some(domain) = domain {
        graph.nodes.retain(|node| node.domain == domain);
    }
    let nodes = graph
        .nodes
        .iter()
        .map(|node| node.key)
        .collect::<BTreeSet<_>>();
    graph.edges.retain(|edge| {
        edge.weight >= minimum && nodes.contains(&edge.source) && nodes.contains(&edge.target)
    });
    Ok(graph)
}

fn identity(state: &Graph, key: i64) -> String {
    state
        .nodes
        .iter()
        .find(|node| node.key == key)
        .map(|node| node.identity())
        .unwrap_or_else(|| format!("#{key}"))
}

fn names(map: &Graph, keys: Vec<i64>) -> Vec<String> {
    let rank = map
        .nodes
        .iter()
        .map(|node| (node.key, node.identity()))
        .collect::<BTreeMap<_, _>>();
    keys.into_iter()
        .map(|key| rank.get(&key).cloned().unwrap_or_else(|| format!("#{key}")))
        .collect()
}
