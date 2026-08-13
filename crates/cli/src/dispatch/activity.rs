use crate::output;
use concord_core::Estate;
use concord_core::activity::Operator;

pub(crate) struct Run {
    operator: concord_core::Result<Option<Operator>>,
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
        if matches!(self.operator, Ok(None)) {
            return;
        }
        let node = match estate.node(task).await {
            Ok(node) => node,
            Err(_) => return,
        };
        let operator = match &self.operator {
            Ok(Some(operator)) => operator,
            Err(error) => {
                output::unavailable(&node.identity(), error, self.json);
                return;
            }
            Ok(None) => unreachable!("absent operator returned before Task lookup"),
        };
        match estate.touch(&node, operator, operation) {
            Ok(activity) => output::activity(&activity, self.json),
            Err(error) => output::unavailable(&node.identity(), &error, self.json),
        }
    }
}
