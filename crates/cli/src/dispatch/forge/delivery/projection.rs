use super::super::github::envelope;
use super::super::projection::{PageRequest, Projection, fault};
use concord_core::acceptance::{Report, Target};
use concord_core::{Coordinate, Error, Result};
use serde::Deserialize;

const LABELS: &str = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){issue(number:$number){labels(first:100){nodes{name} pageInfo{hasNextPage}}}}}";

#[derive(Deserialize)]
struct Data {
    repository: Option<Repository>,
}

#[derive(Deserialize)]
struct Repository {
    issue: Option<Issue>,
}

#[derive(Deserialize)]
struct Issue {
    labels: Labels,
}

#[derive(Deserialize)]
struct Labels {
    nodes: Vec<Label>,
    #[serde(rename = "pageInfo")]
    page: Page,
}

#[derive(Deserialize)]
struct Label {
    name: String,
}

#[derive(Deserialize)]
struct Page {
    #[serde(rename = "hasNextPage")]
    more: bool,
}

pub struct Observation {
    pub snapshot: plumb::delivery::Snapshot,
    pub outcome: String,
    pub observed: u64,
    pub acceptance: Option<Report>,
}

impl Projection<'_> {
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
        let target = Target::select(&self.labels(coordinate).await?)?;
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
        let args = crate::args::acceptance::Observe {
            issue: format!(
                "{}/{}#{}",
                coordinate.owner, coordinate.repository, coordinate.number
            ),
            command: self.command.to_path_buf(),
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
                sub_issues: references(&raw.sub_issues.nodes)?,
                blocked_by: references(&raw.blocked_by.nodes)?,
                blocking: references(&raw.blocking.nodes)?,
            },
            outcome,
            observed,
            acceptance,
        })
    }
}

impl Projection<'_> {
    async fn labels(&self, coordinate: &Coordinate) -> Result<Vec<String>> {
        let process = self
            .request()
            .args(["api", "graphql", "-f"])
            .arg(format!("query={LABELS}"))
            .args(["-f", &format!("owner={}", coordinate.owner)])
            .args(["-f", &format!("name={}", coordinate.repository)])
            .args(["-F", &format!("number={}", coordinate.number)]);
        let body = self.run(process).await.map_err(fault)?;
        let labels = envelope::<Data>(&body)
            .map_err(fault)?
            .and_then(|data| data.repository)
            .and_then(|repository| repository.issue)
            .ok_or_else(|| {
                Error::typed(
                    "concord.issue.projection.missing",
                    "GitHub Issue labels are not readable",
                )
            })?
            .labels;
        if labels.page.more {
            return Err(Error::typed(
                "concord.delivery.snapshot",
                "Issue delivery snapshot exceeds the native label bound",
            ));
        }
        let names = labels
            .nodes
            .into_iter()
            .map(|label| label.name)
            .collect::<Vec<_>>();
        needs(names.iter().cloned())?;
        Ok(names)
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
