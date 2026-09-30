use super::super::projection::{PageRequest, Projection};
use concord_core::{Coordinate, Error, Result};

pub struct Observation {
    pub snapshot: plumb::delivery::Snapshot,
    pub outcome: String,
    pub observed: u64,
}

impl Projection<'_> {
    pub async fn delivery(&self, coordinate: &Coordinate) -> Result<Observation> {
        let request = PageRequest::first(100);
        let raw = self
            .query(coordinate, &request)
            .await
            .map_err(super::super::projection::fault)?;
        if raw.sub_issues.page_info.has_next_page
            || raw.blocked_by.page_info.has_next_page
            || raw.blocking.page_info.has_next_page
        {
            return Err(Error::typed(
                "concord.delivery.snapshot",
                "Issue delivery snapshot exceeds the native relationship bound",
            ));
        }
        let issue =
            super::super::shape::root(coordinate, &raw).map_err(super::super::projection::fault)?;
        let kind = raw
            .issue_type
            .as_ref()
            .map(|kind| kind.name.trim())
            .filter(|kind| !kind.is_empty())
            .ok_or_else(|| {
                Error::typed(
                    "concord.delivery.snapshot",
                    "GitHub Issue has no enabled native type",
                )
            })?
            .to_string();
        let outcome = super::super::readiness::outcome(&raw.body, &kind).ok_or_else(|| {
            Error::typed(
                "concord.delivery.outcome",
                "GitHub Issue has no non-empty Outcome section",
            )
        })?;
        let observed = super::super::projection::now();
        Ok(Observation {
            snapshot: plumb::delivery::Snapshot {
                node: issue.node,
                repository: format!("{}/{}", issue.coordinate.owner, issue.coordinate.repository),
                number: u64::try_from(issue.coordinate.number).map_err(|_| {
                    Error::typed("concord.delivery.snapshot", "Issue number is not positive")
                })?,
                url: issue.url,
                title: issue.title,
                state: issue.state.to_ascii_uppercase(),
                kind,
                updated: raw.updated_at,
                parent: raw.parent.as_ref().map(reference).transpose()?,
                sub_issues: references(&raw.sub_issues.nodes)?,
                blocked_by: references(&raw.blocked_by.nodes)?,
                blocking: references(&raw.blocking.nodes)?,
            },
            outcome,
            observed,
        })
    }
}

fn references(
    raw: &[super::super::github::RawIssueNode],
) -> Result<Vec<plumb::delivery::Reference>> {
    let mut held = raw.iter().map(reference).collect::<Result<Vec<_>>>()?;
    held.sort();
    Ok(held)
}

fn reference(raw: &super::super::github::RawIssueNode) -> Result<plumb::delivery::Reference> {
    let issue = super::super::shape::issue(raw).map_err(super::super::projection::fault)?;
    Ok(plumb::delivery::Reference {
        repository: format!("{}/{}", issue.coordinate.owner, issue.coordinate.repository),
        number: u64::try_from(issue.coordinate.number).map_err(|_| {
            Error::typed("concord.delivery.snapshot", "Issue number is not positive")
        })?,
        node: issue.node,
        url: issue.url,
        title: issue.title,
        state: issue.state.to_ascii_uppercase(),
    })
}
