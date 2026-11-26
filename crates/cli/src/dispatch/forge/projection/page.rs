use super::super::github::Paging;
use super::{Complete, Connection, Fault, Projection, Request};
use concord_core::Coordinate;
use std::collections::{BTreeMap, BTreeSet};

impl Projection {
    pub(in crate::dispatch::forge) async fn complete(
        &self,
        coordinate: &Coordinate,
        connections: BTreeSet<Connection>,
    ) -> std::result::Result<Complete, Fault> {
        let mut request = Request::first(self.size);
        let mut result = Builder::default();
        let mut active = connections;
        for page in 0..usize::from(self.pages) {
            let raw = self.query(coordinate, &request).await?;
            if result.issue.is_none() {
                result.header(coordinate, &raw)?;
            }
            if active.contains(&Connection::Children) {
                super::super::shape::issues(&mut result.children, &raw.children.nodes)?;
                settle(
                    Connection::Children,
                    &raw.children.info,
                    &mut request.children,
                    &mut active,
                )?;
            }
            if active.contains(&Connection::Blockers) {
                super::super::shape::issues(&mut result.blockers, &raw.blockers.nodes)?;
                settle(
                    Connection::Blockers,
                    &raw.blockers.info,
                    &mut request.blockers,
                    &mut active,
                )?;
            }
            if active.contains(&Connection::Blocking) {
                super::super::shape::issues(&mut result.blocking, &raw.blocking.nodes)?;
                settle(
                    Connection::Blocking,
                    &raw.blocking.info,
                    &mut request.blocking,
                    &mut active,
                )?;
            }
            if active.contains(&Connection::Pulls) {
                super::super::shape::collect(&mut result.pulls, &raw.timeline.nodes)?;
                settle(
                    Connection::Pulls,
                    &raw.timeline.info,
                    &mut request.pulls,
                    &mut active,
                )?;
            }
            if active.contains(&Connection::Comments) {
                for comment in &raw.comments.nodes {
                    result.comments.insert(comment.url.clone(), comment.clone());
                }
                settle(
                    Connection::Comments,
                    &raw.comments.info,
                    &mut request.comments,
                    &mut active,
                )?;
            }
            if active.is_empty() {
                return Ok(result.finish());
            }
            if page + 1 == usize::from(self.pages) {
                break;
            }
        }
        Err(super::super::github::provider(
            "page-limit",
            format!(
                "GitHub Issue projection exceeds the {} page limit",
                self.pages
            ),
        ))
    }
}

#[derive(Default)]
struct Builder {
    issue: Option<super::Issue>,
    kind: String,
    body: String,
    parent: Option<super::Issue>,
    children: BTreeMap<String, super::Issue>,
    blockers: BTreeMap<String, super::Issue>,
    blocking: BTreeMap<String, super::Issue>,
    pulls: BTreeMap<String, super::Pull>,
    comments: BTreeMap<String, super::super::github::Comment>,
}

impl Builder {
    fn header(
        &mut self,
        coordinate: &Coordinate,
        raw: &super::super::github::Snapshot,
    ) -> std::result::Result<(), Fault> {
        self.issue = Some(super::super::shape::root(coordinate, raw)?);
        self.kind = raw
            .kind
            .as_ref()
            .map(|kind| kind.name.trim())
            .filter(|kind| !kind.is_empty())
            .ok_or_else(|| {
                super::super::github::provider("type", "GitHub Issue has no enabled native type")
            })?
            .to_string();
        self.body.clone_from(&raw.body);
        self.parent = raw
            .parent
            .as_ref()
            .map(super::super::shape::issue)
            .transpose()?;
        Ok(())
    }

    fn finish(self) -> Complete {
        Complete {
            issue: self.issue.expect("first provider page sets Issue"),
            kind: self.kind,
            body: self.body,
            parent: self.parent,
            children: self.children.into_values().collect(),
            blockers: self.blockers.into_values().collect(),
            blocking: self.blocking.into_values().collect(),
            pulls: self.pulls.into_values().collect(),
            comments: self.comments.into_values().collect(),
        }
    }
}

fn settle(
    connection: Connection,
    page: &Paging,
    cursor: &mut Option<String>,
    active: &mut BTreeSet<Connection>,
) -> std::result::Result<(), Fault> {
    if !page.more {
        active.remove(&connection);
        return Ok(());
    }
    let end = page
        .cursor
        .as_ref()
        .filter(|cursor| !cursor.is_empty())
        .ok_or_else(|| {
            super::super::github::provider(
                "cursor",
                format!(
                    "GitHub {} page has no continuation cursor",
                    name(connection)
                ),
            )
        })?;
    if cursor.as_ref() == Some(end) {
        return Err(super::super::github::provider(
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
        Connection::Children => "sub-issues",
        Connection::Blockers => "blocked-by",
        Connection::Blocking => "blocking",
        Connection::Pulls => "pulls",
        Connection::Comments => "comments",
    }
}
