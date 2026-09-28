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

    pub(crate) async fn touch(&self, estate: &Estate, issue: &str, operation: &str) {
        let Ok(coordinate) = Coordinate::parse(issue) else {
            return;
        };
        let Ok(anchor) = estate.issue(&coordinate).await else {
            return;
        };
        match estate.touch_issue(&anchor, self.operator.as_ref(), operation) {
            Ok(activity) => output::issue_activity(&activity, self.json),
            Err(error) => output::unavailable(&coordinate.identity(), &error, self.json),
        }
    }
}
