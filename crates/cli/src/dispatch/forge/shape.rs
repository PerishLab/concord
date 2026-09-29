use super::github::{RawConnection, RawEvent, RawIssue, RawIssueNode, RawSource};
use super::projection::{Fault, Issue, Page, Pull};
use concord_core::{Coordinate, Result};
use std::collections::BTreeMap;

pub(super) fn root(coordinate: &Coordinate, raw: &RawIssue) -> std::result::Result<Issue, Fault> {
    let issue = issue(&RawIssueNode {
        id: raw.id.clone(),
        number: raw.number,
        url: raw.url.clone(),
        title: raw.title.clone(),
        state: raw.state.clone(),
        repository: raw.repository.clone(),
    })?;
    if &issue.coordinate != coordinate {
        return Err(super::github::provider(
            "coordinate",
            "GitHub Issue reply does not match the requested coordinate",
        ));
    }
    if raw.updated_at.is_empty() {
        return Err(super::github::provider(
            "malformed",
            "GitHub Issue has no update time",
        ));
    }
    Ok(issue)
}

pub(super) fn issue(raw: &RawIssueNode) -> std::result::Result<Issue, Fault> {
    let (owner, repository) = raw
        .repository
        .name_with_owner
        .split_once('/')
        .filter(|(owner, repository)| !owner.is_empty() && !repository.is_empty())
        .ok_or_else(|| {
            super::github::provider(
                "coordinate",
                "GitHub Issue repository coordinate is malformed",
            )
        })?;
    let coordinate = Coordinate {
        owner: owner.to_string(),
        repository: repository.to_string(),
        number: raw.number,
    };
    validate(raw, &coordinate)?;
    Ok(Issue {
        node: raw.id.clone(),
        coordinate,
        url: raw.url.clone(),
        title: raw.title.clone(),
        state: raw.state.to_ascii_lowercase(),
    })
}

fn validate(raw: &RawIssueNode, coordinate: &Coordinate) -> std::result::Result<(), Fault> {
    let expected = format!(
        "https://github.com/{}/{}/issues/{}",
        coordinate.owner, coordinate.repository, coordinate.number
    );
    if raw.id.trim().is_empty() {
        return Err(super::github::provider(
            "malformed",
            "GitHub Issue node is empty",
        ));
    }
    if raw.url != expected {
        return Err(super::github::provider(
            "malformed",
            "GitHub Issue URL is invalid",
        ));
    }
    if raw.title.trim().is_empty() {
        return Err(super::github::provider(
            "malformed",
            "GitHub Issue title is empty",
        ));
    }
    if !matches!(raw.state.to_ascii_lowercase().as_str(), "open" | "closed") {
        return Err(super::github::provider(
            "malformed",
            "GitHub Issue state is invalid",
        ));
    }
    Ok(())
}

pub(super) fn issues(
    target: &mut BTreeMap<String, Issue>,
    raw: &[RawIssueNode],
) -> std::result::Result<(), Fault> {
    for raw in raw {
        let issue = issue(raw)?;
        target.insert(super::projection::key(&issue.coordinate), issue);
    }
    Ok(())
}

pub(super) fn page(
    raw: &RawConnection<RawIssueNode>,
    after: Option<String>,
) -> Result<Page<Issue>> {
    Ok(Page {
        requested_after: after,
        end_cursor: raw.page_info.end_cursor.clone(),
        has_next_page: raw.page_info.has_next_page,
        provider_total: raw.total_count,
        nodes: raw
            .nodes
            .iter()
            .map(issue)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(super::projection::fault)?,
    })
}

pub(super) fn pulls(raw: &RawConnection<RawEvent>, after: Option<String>) -> Result<Page<Pull>> {
    let mut nodes = raw
        .nodes
        .iter()
        .filter_map(|event| event.source.as_ref())
        .filter(|source| source.kind == "PullRequest")
        .map(pull)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(super::projection::fault)?;
    nodes.sort_by(|left, right| {
        (&left.owner, &left.repository, left.number).cmp(&(
            &right.owner,
            &right.repository,
            right.number,
        ))
    });
    Ok(Page {
        requested_after: after,
        end_cursor: raw.page_info.end_cursor.clone(),
        has_next_page: raw.page_info.has_next_page,
        provider_total: raw.total_count,
        nodes,
    })
}

pub(super) fn insert_pulls(
    target: &mut BTreeMap<String, Pull>,
    raw: &[RawEvent],
) -> std::result::Result<(), Fault> {
    for source in raw
        .iter()
        .filter_map(|event| event.source.as_ref())
        .filter(|source| source.kind == "PullRequest")
    {
        let pull = pull(source)?;
        target.insert(pull.node.clone(), pull);
    }
    Ok(())
}

fn pull(raw: &RawSource) -> std::result::Result<Pull, Fault> {
    let repository = raw
        .repository
        .as_ref()
        .ok_or_else(|| malformed("GitHub pull reference has no repository"))?;
    let (owner, repository) = repository
        .name_with_owner
        .split_once('/')
        .ok_or_else(|| malformed("GitHub pull repository coordinate is malformed"))?;
    let number = raw
        .number
        .ok_or_else(|| malformed("GitHub pull reference has no number"))?;
    let node = required(raw.id.as_deref(), "GitHub pull reference has no node")?;
    let url = required(raw.url.as_deref(), "GitHub pull reference has no URL")?;
    let title = required(raw.title.as_deref(), "GitHub pull reference has no title")?;
    let state =
        required(raw.state.as_deref(), "GitHub pull reference has no state")?.to_ascii_lowercase();
    let updated = required(
        raw.updated_at.as_deref(),
        "GitHub pull reference has no update time",
    )?;
    let expected = format!("https://github.com/{owner}/{repository}/pull/{number}");
    if url != expected || !matches!(state.as_str(), "open" | "closed" | "merged") {
        return Err(malformed("GitHub pull reference is malformed"));
    }
    Ok(Pull {
        node: node.to_string(),
        owner: owner.to_string(),
        repository: repository.to_string(),
        number,
        url: url.to_string(),
        title: title.to_string(),
        state,
        merged_at: raw.merged_at.clone(),
        updated: updated.to_string(),
    })
}

fn required<'a>(value: Option<&'a str>, message: &str) -> std::result::Result<&'a str, Fault> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| malformed(message))
}

fn malformed(message: &str) -> Fault {
    super::github::provider("malformed", message)
}
