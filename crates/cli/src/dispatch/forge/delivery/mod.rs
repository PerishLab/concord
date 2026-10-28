mod adaptor;
mod branches;
mod flow;
pub(super) mod git;
mod handoff;
mod progress;
pub(super) mod pull;
mod settle;

use crate::args::issue::Delivery;
use crate::dispatch::Completion;
use concord_core::{Error, Estate, Result};

pub async fn run(estate: &Estate, command: Delivery, output: bool) -> Result<Completion> {
    let forge = match &command {
        Delivery::Prepare { forge, .. } | Delivery::Land { forge, .. } => forge,
    };
    adaptor::select(forge)?;
    match command {
        Delivery::Prepare {
            issue,
            authority,
            revision,
            base,
            handoff,
            command,
            timeout,
            forge: _,
        } => handoff::prepare(
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
        .map(|()| Completion::Done),
        command @ Delivery::Land { .. } => flow::land(estate, command, output).await,
    }
}

pub(super) fn version(value: u64) -> Result<()> {
    if matches!(value, 1 | 2) {
        return Ok(());
    }
    Err(Error::typed(
        "concord.delivery.envelope",
        "delivery envelope version must be 1 (legacy GitHub) or 2 (bound adaptor)",
    ))
}

pub(super) const SHAPE: &str = r#"{"version":2,"forge":{"kind":"github","host":"github.com","repository":"OWNER/REPOSITORY","node":"R_node"},"plan":{"schema":"concord.issue-member-delivery/v6","issue":{"owner":"OWNER","repository":"REPOSITORY","number":1},"node":"I_node","revision":1,"member":{},"boundary":{},"authority":{"kind":"plumb"},"delivery":{},"acceptance":null}}"#;

#[cfg(test)]
mod tests {
    #[test]
    fn version() {
        super::version(1).expect("legacy GitHub");
        super::version(2).expect("bound adaptor");
        assert_eq!(
            super::version(3).expect_err("other version").code(),
            "concord.delivery.envelope"
        );
    }
}
