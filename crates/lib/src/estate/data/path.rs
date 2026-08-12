use super::{Flow, Graph, Weight};
use crate::{Error, Result};

impl Graph {
    pub fn paths(
        &self,
        source: i64,
        target: i64,
        minimum: Weight,
        limit: usize,
    ) -> Result<Vec<Vec<i64>>> {
        let mut found = Vec::new();
        let mut stack = vec![(source, vec![source])];
        while let Some((node, path)) = stack.pop() {
            if node == target {
                found.push(path);
                if found.len() > limit {
                    return Err(Error::typed(
                        "concord.graph.limit",
                        format!("directed path result exceeds {limit}"),
                    ));
                }
                continue;
            }
            for next in self.steps(node, Flow::Out, minimum).into_iter().rev() {
                if !path.contains(&next) {
                    let mut branch = path.clone();
                    branch.push(next);
                    stack.push((next, branch));
                }
            }
        }
        Ok(found)
    }
}
