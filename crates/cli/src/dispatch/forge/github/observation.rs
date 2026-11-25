use super::super::projection::{PageRequest, Projection, fault};
use super::{Fault, RawIssue, identity, provider};
use crate::runtime::github;
use concord_core::acceptance::{Report, Target};
use concord_core::{Coordinate, Error, Result};

pub struct Observation {
    pub snapshot: plumb::delivery::Snapshot,
    pub outcome: String,
    pub observed: u64,
    pub acceptance: Option<Report>,
}

impl Projection {
    pub async fn delivery(&self, coordinate: &Coordinate) -> Result<Observation> {
        let request = PageRequest::first(100);
        let raw = self.query(coordinate, &request).await.map_err(fault)?;
        if raw.sub_issues.page_info.has_next_page
            || raw.blocked_by.page_info.has_next_page
            || raw.blocking.page_info.has_next_page
        {
            return Err(Error::typed(
                "concord.delivery.snapshot",
                "Issue delivery snapshot exceeds the native relationship bound",
            ));
        }
        let issue = super::super::shape::root(coordinate, &raw).map_err(fault)?;
        let target = Target::select(&self.labels(coordinate, &issue.node).await?)?;
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
            let section = super::super::readiness::heading(&kind);
            let heading = if section == "expected outcome" {
                "Expected outcome"
            } else {
                "Outcome"
            };
            Error::detailed(
                "concord.delivery.outcome",
                format!(
                    "GitHub Issue type {kind} requires a non-empty {heading} section; add `## {heading}` or `### {heading}` with the required outcome text"
                ),
                serde_json::json!({"kind": kind, "section": section}),
            )
        })?;
        let args = crate::args::acceptance::Observe {
            issue: format!(
                "{}/{}#{}",
                coordinate.owner, coordinate.repository, coordinate.number
            ),
            timeout: self.timeout,
            pages: usize::from(self.max_pages),
        };
        let reviewed = crate::dispatch::acceptance::review(&args).await?;
        if !reviewed.current(&issue.node, &kind, &raw.body) || reviewed.evaluation.target != target
        {
            return Err(Error::typed(
                "concord.acceptance.changed",
                "delivery snapshot and acceptance basis differ",
            ));
        }
        let acceptance = Some(reviewed.report()?);
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
                children: references(&raw.sub_issues.nodes)?,
                blockers: references(&raw.blocked_by.nodes)?,
                blocking: references(&raw.blocking.nodes)?,
            },
            outcome,
            observed,
            acceptance,
        })
    }

    async fn labels(&self, coordinate: &Coordinate, node: &str) -> Result<Vec<String>> {
        let names = self.names(coordinate, node)?;
        needs(names.iter().cloned())?;
        Ok(names)
    }

    fn names(&self, coordinate: &Coordinate, node: &str) -> Result<Vec<String>> {
        use runseal::github::{Target, Window};
        let scope = format!("{}/{}", coordinate.owner, coordinate.repository);
        let native = identity::Native::open(&scope, self.timeout)?;
        let number = u32::try_from(coordinate.number)
            .map_err(|_| Error::typed("concord.delivery.snapshot", "invalid Issue number"))?;
        let target = Target::new(&coordinate.owner, &coordinate.repository, number)
            .map_err(identity::diagnostic)?;
        let window = Window::new(100, None).map_err(identity::diagnostic)?;
        let page = github::call("issue.labels", || native.client.labels(&target, &window))
            .map_err(identity::diagnostic)?
            .value;
        if page.target != node || page.next.is_some() || page.rows.len() != page.total as usize {
            return Err(Error::typed(
                "concord.delivery.snapshot",
                "Issue delivery labels are incomplete or belong to another Issue",
            ));
        }
        Ok(page.rows.into_iter().map(|label| label.name).collect())
    }

    pub(super) fn snapshot(
        &self,
        coordinate: &Coordinate,
        request: &PageRequest,
    ) -> std::result::Result<RawIssue, Fault> {
        use runseal::github::{Target, snapshot::Cursors};
        let scope = format!("{}/{}", coordinate.owner, coordinate.repository);
        let native = identity::Native::open(&scope, self.timeout)
            .map_err(|error| provider("provider", error.message()))?;
        let number = u32::try_from(coordinate.number)
            .map_err(|_| provider("malformed", "invalid Issue number"))?;
        let size = u8::try_from(request.size)
            .map_err(|_| provider("page-limit", "invalid Issue page size"))?;
        let target = Target::new(&coordinate.owner, &coordinate.repository, number)
            .map_err(|error| provider("malformed", error.to_string()))?;
        let cursors = Cursors {
            children: request.sub_issues_after.clone(),
            blockers: request.blocked_by_after.clone(),
            blocking: request.blocking_after.clone(),
            pulls: request.pulls_after.clone(),
            comments: request.comments_after.clone(),
        };
        let receipt = github::call("issue.snapshot", || {
            native.client.snapshot(&target, size, &cursors)
        })
        .map_err(|error| provider("provider", identity::diagnostic(error).message()))?;
        let bytes = serde_json::to_vec(&receipt.value)
            .map_err(|_| provider("malformed", "GitHub Issue projection could not be encoded"))?;
        if bytes.len() > super::LIMIT {
            return Err(provider(
                "malformed",
                "GitHub Issue projection exceeds the reply limit",
            ));
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            provider(
                "malformed",
                "GitHub Issue projection does not match Concord facts",
            )
        })
    }
}

fn needs(names: impl Iterator<Item = String>) -> Result<()> {
    let mut held = names
        .filter(|name| name.starts_with("needs:"))
        .collect::<Vec<_>>();
    if held.is_empty() {
        return Ok(());
    }
    held.sort();
    held.dedup();
    Err(Error::detailed(
        "concord.issue.needs",
        format!(
            "Issue carries {}; resolve it and remove the label before working it",
            held.join(", ")
        ),
        serde_json::json!({"labels": held}),
    ))
}

fn references(
    raw: &[super::super::github::RawIssueNode],
) -> Result<Vec<plumb::delivery::Reference>> {
    let mut held = raw.iter().map(reference).collect::<Result<Vec<_>>>()?;
    held.sort();
    Ok(held)
}

fn reference(raw: &super::super::github::RawIssueNode) -> Result<plumb::delivery::Reference> {
    let issue = super::super::shape::issue(raw).map_err(fault)?;
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
