use super::github::RawPageInfo;
use super::projection::{CompleteIssue, Connection, Fault, PageRequest, Projection};
use concord_core::Coordinate;
use std::collections::{BTreeMap, BTreeSet};

impl Projection<'_> {
    pub(super) async fn complete(
        &self,
        coordinate: &Coordinate,
        connections: BTreeSet<Connection>,
    ) -> std::result::Result<CompleteIssue, Fault> {
        let mut request = PageRequest::first(self.page_size);
        let mut result = Builder::default();
        let mut active = connections;
        for page in 0..usize::from(self.max_pages) {
            let raw = self.query(coordinate, &request).await?;
            if result.issue.is_none() {
                result.header(coordinate, &raw)?;
            }
            if active.contains(&Connection::SubIssues) {
                super::shape::issues(&mut result.sub_issues, &raw.sub_issues.nodes)?;
                settle(
                    Connection::SubIssues,
                    &raw.sub_issues.page_info,
                    &mut request.sub_issues_after,
                    &mut active,
                )?;
            }
            if active.contains(&Connection::BlockedBy) {
                super::shape::issues(&mut result.blocked_by, &raw.blocked_by.nodes)?;
                settle(
                    Connection::BlockedBy,
                    &raw.blocked_by.page_info,
                    &mut request.blocked_by_after,
                    &mut active,
                )?;
            }
            if active.contains(&Connection::Blocking) {
                super::shape::issues(&mut result.blocking, &raw.blocking.nodes)?;
                settle(
                    Connection::Blocking,
                    &raw.blocking.page_info,
                    &mut request.blocking_after,
                    &mut active,
                )?;
            }
            if active.contains(&Connection::Comments) {
                for comment in &raw.comments.nodes {
                    result.comments.insert(comment.url.clone(), comment.clone());
                }
                settle(
                    Connection::Comments,
                    &raw.comments.page_info,
                    &mut request.comments_after,
                    &mut active,
                )?;
            }
            if active.is_empty() {
                return Ok(result.finish());
            }
            if page + 1 == usize::from(self.max_pages) {
                break;
            }
        }
        Err(super::github::provider(
            "page-limit",
            format!(
                "GitHub Issue projection exceeds the {} page limit",
                self.max_pages
            ),
        ))
    }
}

#[derive(Default)]
struct Builder {
    issue: Option<super::projection::Issue>,
    kind: String,
    body: String,
    parent: Option<super::projection::Issue>,
    sub_issues: BTreeMap<String, super::projection::Issue>,
    blocked_by: BTreeMap<String, super::projection::Issue>,
    blocking: BTreeMap<String, super::projection::Issue>,
    comments: BTreeMap<String, super::github::RawComment>,
}

impl Builder {
    fn header(
        &mut self,
        coordinate: &Coordinate,
        raw: &super::github::RawIssue,
    ) -> std::result::Result<(), Fault> {
        self.issue = Some(super::shape::root(coordinate, raw)?);
        self.kind = raw
            .issue_type
            .as_ref()
            .map(|kind| kind.name.trim())
            .filter(|kind| !kind.is_empty())
            .ok_or_else(|| {
                super::github::provider("type", "GitHub Issue has no enabled native type")
            })?
            .to_string();
        self.body.clone_from(&raw.body);
        self.parent = raw.parent.as_ref().map(super::shape::issue).transpose()?;
        Ok(())
    }

    fn finish(self) -> CompleteIssue {
        CompleteIssue {
            issue: self.issue.expect("first provider page sets Issue"),
            kind: self.kind,
            body: self.body,
            parent: self.parent,
            sub_issues: self.sub_issues.into_values().collect(),
            blocked_by: self.blocked_by.into_values().collect(),
            blocking: self.blocking.into_values().collect(),
            comments: self.comments.into_values().collect(),
        }
    }
}

fn settle(
    connection: Connection,
    page: &RawPageInfo,
    cursor: &mut Option<String>,
    active: &mut BTreeSet<Connection>,
) -> std::result::Result<(), Fault> {
    if !page.has_next_page {
        active.remove(&connection);
        return Ok(());
    }
    let end = page
        .end_cursor
        .as_ref()
        .filter(|cursor| !cursor.is_empty())
        .ok_or_else(|| {
            super::github::provider(
                "cursor",
                format!(
                    "GitHub {} page has no continuation cursor",
                    name(connection)
                ),
            )
        })?;
    if cursor.as_ref() == Some(end) {
        return Err(super::github::provider(
            "cursor",
            format!(
                "GitHub {} continuation cursor did not advance",
                name(connection)
            ),
        ));
    }
    *cursor = Some(end.clone());
    Ok(())
}

fn name(connection: Connection) -> &'static str {
    match connection {
        Connection::SubIssues => "sub-issues",
        Connection::BlockedBy => "blocked-by",
        Connection::Blocking => "blocking",
        Connection::Comments => "comments",
    }
}
