use super::{fault, model::Observation};
use crate::args::close::Observe;
use crate::dispatch::forge::github::lifecycle;
use concord_core::{Coordinate, Result};

pub(super) struct Reader<'a> {
    pub args: &'a Observe,
    pub coordinate: Coordinate,
}

impl Reader<'_> {
    pub async fn observe(&self) -> Result<Observation> {
        let before = lifecycle::read(self.args, &self.coordinate).await?;
        before.issue.ordinary()?;
        let args = crate::args::acceptance::Observe {
            issue: self.args.issue.clone(),
            command: self.args.command.clone(),
            timeout: self.args.timeout,
            pages: usize::from(self.args.pages),
        };
        let reviewed = crate::dispatch::acceptance::review(&args).await?;
        let after = lifecycle::read(self.args, &self.coordinate).await?;
        if before != after
            || reviewed.repository() != after.issue.repository
            || !reviewed.current(&after.issue.node, &after.issue.kind, &after.body)
        {
            return Err(fault(
                "changed",
                "Issue and acceptance basis changed during closure observation",
            ));
        }
        Ok(Observation {
            issue: after.issue,
            labels: reviewed.labels().to_vec(),
            acceptance: reviewed.evaluation,
        })
    }

    pub async fn close(&self, issue: &super::model::Issue) -> Result<()> {
        lifecycle::close(self.args, issue).await
    }
}
