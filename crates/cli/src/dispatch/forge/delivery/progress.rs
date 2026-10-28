use super::adaptor;
use crate::dispatch::Completion;
use concord_core::issue_delivery::Resume;
use concord_core::{Error, Result};

pub(super) fn pending(
    plan: &concord_core::issue_delivery::Plan,
    pull: i64,
    output: bool,
) -> Result<Completion> {
    crate::dispatch::emit(
        serde_json::json!({
            "schema": "concord.issue-delivery-pending/v1",
            "status": "pending",
            "issue": plan.issue,
            "candidate": plan.delivery.candidate,
            "pull": adaptor::reference(&plan.delivery.repository, pull)?,
            "cookbook": "concord.delivery.pending",
        }),
        output,
    )?;
    Ok(Completion::Pending)
}

pub(super) fn active(resumed: &Resume) -> Result<()> {
    if resumed.released {
        return Err(Error::typed(
            "concord.delivery.stale",
            "released delivery cannot create or mutate a pull request",
        ));
    }
    Ok(())
}
