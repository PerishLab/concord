use super::projection::{self, Connection, Diagnostic, Edge, EdgeKind, Graph, Issue, Projection};
use concord_core::Coordinate;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Instant;

impl Projection<'_> {
    pub async fn graph(&self, coordinate: &Coordinate, max_nodes: u16) -> Graph {
        let started = Instant::now();
        let mut walk = Walk::new(coordinate, max_nodes);
        while let Some(current) = walk.queue.pop_front() {
            let subject = projection::key(&current);
            let held = self
                .complete(
                    &current,
                    BTreeSet::from([
                        Connection::SubIssues,
                        Connection::BlockedBy,
                        Connection::Blocking,
                    ]),
                )
                .await;
            let held = match held {
                Ok(held) => held,
                Err(error) => {
                    walk.fault(subject, error.code, error.message);
                    continue;
                }
            };
            walk.nodes.insert(subject.clone(), held.issue.clone());
            let mut related = relationships(&subject, held);
            related.sort_by_key(|(issue, _)| projection::key(&issue.coordinate));
            for (issue, edge) in related {
                walk.admit(issue, edge);
            }
        }
        cycles(&walk.edges, &mut walk.diagnostics);
        let graph = walk.finish(coordinate);
        projection::record(
            "issue.graph",
            started.elapsed(),
            if graph.complete {
                "fresh"
            } else {
                "incomplete"
            },
        );
        graph
    }
}

struct Walk {
    max_nodes: u16,
    queue: VecDeque<Coordinate>,
    queued: BTreeSet<String>,
    nodes: BTreeMap<String, Issue>,
    edges: BTreeSet<Edge>,
    diagnostics: BTreeSet<Diagnostic>,
}

impl Walk {
    fn new(root: &Coordinate, max_nodes: u16) -> Self {
        Self {
            max_nodes,
            queue: VecDeque::from([root.clone()]),
            queued: BTreeSet::from([projection::key(root)]),
            nodes: BTreeMap::new(),
            edges: BTreeSet::new(),
            diagnostics: BTreeSet::new(),
        }
    }

    fn fault(&mut self, subject: String, code: &str, message: String) {
        self.diagnostics.insert(Diagnostic {
            code: code.to_string(),
            subject,
            message,
        });
    }

    fn admit(&mut self, issue: Issue, edge: Edge) {
        let key = projection::key(&issue.coordinate);
        if self.queued.contains(&key) {
            self.nodes.entry(key).or_insert(issue);
            self.edges.insert(edge);
            return;
        }
        if self.queued.len() >= usize::from(self.max_nodes) {
            self.fault(
                key,
                "node-limit",
                format!("Issue graph exceeds the {} node limit", self.max_nodes),
            );
            return;
        }
        self.queued.insert(key.clone());
        self.queue.push_back(issue.coordinate.clone());
        self.nodes.insert(key, issue);
        self.edges.insert(edge);
    }

    fn finish(self, root: &Coordinate) -> Graph {
        let mut nodes = self.nodes.into_values().collect::<Vec<_>>();
        nodes.sort_by_key(|issue| projection::key(&issue.coordinate));
        Graph {
            schema: projection::GRAPH,
            root: root.clone(),
            nodes,
            edges: self.edges.into_iter().collect(),
            complete: self.diagnostics.is_empty(),
            diagnostics: self.diagnostics.into_iter().collect(),
            observed_at: projection::now(),
        }
    }
}

fn relationships(current: &str, held: projection::CompleteIssue) -> Vec<(Issue, Edge)> {
    let mut related = Vec::new();
    if let Some(parent) = held.parent {
        related.push((
            parent.clone(),
            edge(
                EdgeKind::Parent,
                &projection::key(&parent.coordinate),
                current,
            ),
        ));
    }
    for child in held.sub_issues {
        related.push((
            child.clone(),
            edge(
                EdgeKind::Parent,
                current,
                &projection::key(&child.coordinate),
            ),
        ));
    }
    for blocker in held.blocked_by {
        related.push((
            blocker.clone(),
            edge(
                EdgeKind::Blocking,
                &projection::key(&blocker.coordinate),
                current,
            ),
        ));
    }
    for blocked in held.blocking {
        related.push((
            blocked.clone(),
            edge(
                EdgeKind::Blocking,
                current,
                &projection::key(&blocked.coordinate),
            ),
        ));
    }
    related
}

fn edge(kind: EdgeKind, source: &str, target: &str) -> Edge {
    Edge {
        kind,
        source: source.to_string(),
        target: target.to_string(),
    }
}

fn cycles(edges: &BTreeSet<Edge>, diagnostics: &mut BTreeSet<Diagnostic>) {
    let mut degree = BTreeMap::<String, usize>::new();
    let mut next = BTreeMap::<String, BTreeSet<String>>::new();
    for edge in edges.iter().filter(|edge| edge.kind == EdgeKind::Blocking) {
        degree.entry(edge.source.clone()).or_default();
        *degree.entry(edge.target.clone()).or_default() += 1;
        next.entry(edge.source.clone())
            .or_default()
            .insert(edge.target.clone());
    }
    let mut queue = degree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| node.clone())
        .collect::<VecDeque<_>>();
    while let Some(node) = queue.pop_front() {
        if let Some(targets) = next.get(&node) {
            settle(targets, &mut degree, &mut queue);
        }
    }
    let cycle = degree
        .into_iter()
        .filter(|(_, degree)| *degree > 0)
        .map(|(node, _)| node)
        .collect::<Vec<_>>();
    if !cycle.is_empty() {
        diagnostics.insert(Diagnostic {
            code: "dependency-cycle".to_string(),
            subject: cycle.join(","),
            message: "native GitHub blocking relationships contain a cycle".to_string(),
        });
    }
}

fn settle(
    targets: &BTreeSet<String>,
    degree: &mut BTreeMap<String, usize>,
    queue: &mut VecDeque<String>,
) {
    for target in targets {
        let Some(held) = degree.get_mut(target) else {
            continue;
        };
        *held -= 1;
        if *held == 0 {
            queue.push_back(target.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cycles as detect;
    use crate::dispatch::forge::projection::{Diagnostic, Edge, EdgeKind};
    use std::collections::BTreeSet;

    #[test]
    fn cycles() {
        let mut diagnostics = BTreeSet::<Diagnostic>::new();
        let edges = BTreeSet::from([
            Edge {
                kind: EdgeKind::Parent,
                source: "a".into(),
                target: "b".into(),
            },
            Edge {
                kind: EdgeKind::Blocking,
                source: "b".into(),
                target: "c".into(),
            },
            Edge {
                kind: EdgeKind::Blocking,
                source: "c".into(),
                target: "b".into(),
            },
        ]);
        detect(&edges, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics.iter().next().expect("cycle").code,
            "dependency-cycle"
        );
    }
}
