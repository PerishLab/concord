mod client;
mod flow;
mod git;
mod model;
mod projection;
mod pull;
mod settle;
#[cfg(test)]
mod squash;

use super::super::emit;
use super::projection::Projection;
use crate::args::issue::Delivery;
use concord_core::authority::Plumb;
use concord_core::{Coordinate, Error, Estate, Result, issue_delivery};
use serde_json::json;

pub async fn run(estate: &Estate, command: Delivery, output: bool) -> Result<()> {
    match command {
        Delivery::Prepare {
            issue,
            revision,
            base,
            command,
            timeout,
        } => {
            let issue = Coordinate::parse(&issue)?;
            let observed = Projection::new(&command, 100, 1, timeout)
                .delivery(&issue)
                .await?;
            let plan = issue_delivery::prepare::<Plumb>(
                estate,
                &issue_delivery::Request {
                    issue,
                    revision,
                    base,
                    snapshot: observed.snapshot,
                    observed: observed.observed,
                    outcome: observed.outcome,
                },
            )
            .await?;
            emit(json!({"plan": plan}), output)
        }
        command @ Delivery::Land { .. } => flow::land(estate, command, output).await,
    }
}

pub(super) fn version(value: u64) -> Result<()> {
    if value == 1 {
        return Ok(());
    }
    Err(Error::typed(
        "concord.delivery.envelope",
        "delivery envelope version must be 1",
    ))
}

pub(super) const SHAPE: &str = r#"{"version":1,"plan":{"schema":"concord.issue-member-delivery/v4","issue":{"owner":"OWNER","repository":"REPOSITORY","number":1},"node":"I_node","revision":1,"member":{},"boundary":{},"authority":{},"delivery":{}}}"#;

#[cfg(test)]
mod tests {
    #[test]
    fn version() {
        super::version(1).expect("version one");
        assert_eq!(
            super::version(2).expect_err("other version").code(),
            "concord.delivery.envelope"
        );
    }
}
