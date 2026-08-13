use crate::output;
use concord_core::Estate;
use concord_core::activity::Operator;

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
