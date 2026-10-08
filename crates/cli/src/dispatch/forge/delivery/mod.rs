mod branches;
mod client;
mod flow;
mod git;
mod handoff;
mod model;
mod projection;
mod pull;
mod settle;

use crate::args::issue::Delivery;
use concord_core::{Error, Estate, Result};

pub async fn run(estate: &Estate, command: Delivery, output: bool) -> Result<()> {
    match command {
        Delivery::Prepare {
            issue,
            authority,
            revision,
            base,
            handoff,
            command,
            timeout,
        } => {
            handoff::prepare(
                estate,
                handoff::Parameters {
                    issue: &issue,
                    authority,
                    revision,
                    base,
                    carry: handoff,
                    command: &command,
                    timeout,
                },
                output,
            )
            .await
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

pub(super) const SHAPE: &str = r#"{"version":1,"plan":{"schema":"concord.issue-member-delivery/v6","issue":{"owner":"OWNER","repository":"REPOSITORY","number":1},"node":"I_node","revision":1,"member":{},"boundary":{},"authority":{"kind":"plumb"},"delivery":{},"acceptance":null}}"#;

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
