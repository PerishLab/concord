use crate::output;
use concord_core::activity::Operator;
use concord_core::{Coordinate, Estate};

pub(crate) struct Run {
    operator: Option<Operator>,
    json: bool,
}

impl Run {
    pub(crate) fn new(json: bool) -> Self {
        Self {
            operator: Operator::detect(),
            json,
        }
    }

    pub(crate) async fn touch(&self, estate: &Estate, task: &str, operation: &str) {
        if task.contains('#') {
            let Ok(coordinate) = Coordinate::parse(task) else {
                return;
            };
            let Ok(issue) = estate.issue(&coordinate).await else {
                return;
            };
            match estate.touch_issue(&issue, self.operator.as_ref(), operation) {
                Ok(activity) => output::issue_activity(&activity, self.json),
                Err(error) => output::unavailable(&coordinate.identity(), &error, self.json),
            }
            return;
        }
        let node = match estate.node(task).await {
            Ok(node) => node,
            Err(_) => return,
        };
        match estate.touch(&node, self.operator.as_ref(), operation) {
            Ok(activity) => output::activity(&activity, self.json),
            Err(error) => output::unavailable(&node.identity(), &error, self.json),
        }
    }
}
