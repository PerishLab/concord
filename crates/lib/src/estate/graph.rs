use super::World;
use super::{Degree, Estate, Flow, Graph, Life, Node, Weight};
use crate::Result;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[path = "data/path.rs"]
mod route;

impl Estate {
    pub async fn graph(&self, retired: bool) -> Result<Graph> {
        let world = World::load(self).await?;
        let mut nodes = world.nodes.clone();
        let mut edges = self.edges(&world).await?;
        if !retired {
            let active = nodes
                .iter()
                .filter(|node| node.life == Life::Active)
                .map(|node| node.key)
                .collect::<BTreeSet<_>>();
            nodes.retain(|node| active.contains(&node.key));
            edges.retain(|edge| active.contains(&edge.source) && active.contains(&edge.target));
        }
        nodes.sort_by_key(Node::identity);
        let order = nodes
            .iter()
            .enumerate()
            .map(|(rank, node)| (node.key, rank))
            .collect::<BTreeMap<_, _>>();
        edges.sort_by_key(|edge| {
            (
                order.get(&edge.source).copied().unwrap_or(usize::MAX),
                order.get(&edge.target).copied().unwrap_or(usize::MAX),
                edge.key,
            )
        });
        Ok(Graph {
            revision: world.revision,
            nodes,
            edges,
        })
    }
}

impl Graph {
    pub fn adjacency(&self) -> BTreeMap<i64, (Vec<i64>, Vec<i64>)> {
        let mut map = self
            .nodes
            .iter()
            .map(|node| (node.key, (Vec::new(), Vec::new())))
            .collect::<BTreeMap<_, _>>();
        for edge in &self.edges {
            if let Some(pair) = map.get_mut(&edge.source) {
                pair.0.push(edge.target);
            }
            if let Some(pair) = map.get_mut(&edge.target) {
                pair.1.push(edge.source);
            }
        }
        for pair in map.values_mut() {
            pair.0.sort_unstable();
            pair.0.dedup();
            pair.1.sort_unstable();
            pair.1.dedup();
        }
        map
    }

    pub fn neighbors(&self, source: i64, flow: Flow, depth: usize, minimum: Weight) -> Vec<i64> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([(source, 0)]);
        while let Some((node, level)) = queue.pop_front() {
            if level >= depth {
                continue;
            }
            for next in self.steps(node, flow, minimum) {
                if next != source && seen.insert(next) {
                    queue.push_back((next, level + 1));
                }
            }
        }
        self.sorted(seen)
    }

    pub fn reach(&self, source: i64, flow: Flow, minimum: Weight) -> Vec<i64> {
        self.neighbors(source, flow, usize::MAX, minimum)
    }

    pub fn degree(&self, node: i64, minimum: Weight) -> Degree {
        Degree {
            out: self
                .edges
                .iter()
                .filter(|edge| edge.source == node && edge.weight >= minimum)
                .count(),
            r#in: self
                .edges
                .iter()
                .filter(|edge| edge.target == node && edge.weight >= minimum)
                .count(),
        }
    }

    pub fn path(&self, source: i64, target: i64, minimum: Weight) -> Option<Vec<i64>> {
        let mut parent = BTreeMap::from([(source, source)]);
        let mut queue = VecDeque::from([source]);
        while let Some(node) = queue.pop_front() {
            if node == target {
                break;
            }
            for edge in self
                .edges
                .iter()
                .filter(|edge| edge.source == node && edge.weight >= minimum)
            {
                if parent.insert(edge.target, node).is_none() {
                    queue.push_back(edge.target);
                }
            }
        }
        if !parent.contains_key(&target) {
            return None;
        }
        let mut path = vec![target];
        while path.last().copied() != Some(source) {
            path.push(parent[path.last().expect("path has target")]);
        }
        path.reverse();
        Some(path)
    }

    pub fn cycles(&self) -> Vec<Vec<i64>> {
        self.scc()
            .into_iter()
            .filter_map(|group| self.cycle(&group))
            .collect()
    }

    pub fn scc(&self) -> Vec<Vec<i64>> {
        let map = self.adjacency();
        let mut seen = BTreeSet::new();
        let mut order = Vec::new();
        for node in &self.nodes {
            post(node.key, &map, &mut seen, &mut order);
        }
        seen.clear();
        let mut groups = Vec::new();
        for node in order.into_iter().rev() {
            if seen.contains(&node) {
                continue;
            }
            groups.push(self.sorted(gather(node, &map, &mut seen)));
        }
        groups.sort_by_key(|group| {
            group
                .first()
                .and_then(|key| self.rank(*key))
                .unwrap_or(usize::MAX)
        });
        groups
    }

    fn steps(&self, node: i64, flow: Flow, minimum: Weight) -> Vec<i64> {
        let mut out = Vec::new();
        for edge in self.edges.iter().filter(|edge| edge.weight >= minimum) {
            if matches!(flow, Flow::Out | Flow::Both) && edge.source == node {
                out.push(edge.target);
            }
            if matches!(flow, Flow::In | Flow::Both) && edge.target == node {
                out.push(edge.source);
            }
        }
        out.sort_by_key(|key| self.rank(*key).unwrap_or(usize::MAX));
        out.dedup();
        out
    }

    fn sorted(&self, keys: BTreeSet<i64>) -> Vec<i64> {
        let mut keys = keys.into_iter().collect::<Vec<_>>();
        keys.sort_by_key(|key| self.rank(*key).unwrap_or(usize::MAX));
        keys
    }

    fn rank(&self, key: i64) -> Option<usize> {
        self.nodes.iter().position(|node| node.key == key)
    }

    fn cycle(&self, group: &[i64]) -> Option<Vec<i64>> {
        let allowed = group.iter().copied().collect::<BTreeSet<_>>();
        for source in group {
            for edge in self
                .edges
                .iter()
                .filter(|edge| edge.source == *source && allowed.contains(&edge.target))
            {
                if edge.target == *source {
                    return Some(vec![*source, *source]);
                }
                if let Some(mut path) = subset(self, edge.target, *source, &allowed) {
                    path.insert(0, *source);
                    return Some(path);
                }
            }
        }
        None
    }
}

fn post(
    start: i64,
    map: &BTreeMap<i64, (Vec<i64>, Vec<i64>)>,
    seen: &mut BTreeSet<i64>,
    order: &mut Vec<i64>,
) {
    if !seen.insert(start) {
        return;
    }
    let mut stack = vec![(start, false)];
    while let Some((node, done)) = stack.pop() {
        if done {
            order.push(node);
            continue;
        }
        stack.push((node, true));
        for next in map
            .get(&node)
            .map(|pair| pair.0.iter().rev())
            .into_iter()
            .flatten()
        {
            if seen.insert(*next) {
                stack.push((*next, false));
            }
        }
    }
}

fn gather(
    start: i64,
    map: &BTreeMap<i64, (Vec<i64>, Vec<i64>)>,
    seen: &mut BTreeSet<i64>,
) -> BTreeSet<i64> {
    let mut group = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    seen.insert(start);
    while let Some(node) = queue.pop_front() {
        group.insert(node);
        for previous in map.get(&node).map(|pair| pair.1.as_slice()).unwrap_or(&[]) {
            if seen.insert(*previous) {
                queue.push_back(*previous);
            }
        }
    }
    group
}

fn subset(graph: &Graph, source: i64, target: i64, allowed: &BTreeSet<i64>) -> Option<Vec<i64>> {
    let mut parent = BTreeMap::from([(source, source)]);
    let mut queue = VecDeque::from([source]);
    while let Some(node) = queue.pop_front() {
        if node == target {
            break;
        }
        for next in graph.steps(node, Flow::Out, Weight::Unknown) {
            if allowed.contains(&next) && parent.insert(next, node).is_none() {
                queue.push_back(next);
            }
        }
    }
    if !parent.contains_key(&target) {
        return None;
    }
    let mut path = vec![target];
    while path.last().copied() != Some(source) {
        path.push(parent[path.last().expect("path has target")]);
    }
    path.reverse();
    Some(path)
}
