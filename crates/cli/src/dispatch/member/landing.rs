use crate::args::member::Landing;
use concord_core::{Error, Result};

pub(super) fn run(command: &Landing) -> Result<()> {
    let issue = command.issue();
    Err(Error::typed(
        "concord.landing.retired",
        format!(
            "member landing prepare/ready is retired: repository-only plumb land deletes the source branch and detaches the Member before Concord can release it. Use concord issue delivery prepare {issue} --revision <current-revision> --github-command <program> --json, then concord issue delivery land {issue} --plan <prepared-file> --github-command <program>. Issue-led delivery owns current acceptance, exact merge readback, main synchronization, Member release and branch cleanup. Old Member landing plans cannot be used as Issue delivery plans; prepare a fresh plan."
        ),
    ))
}
