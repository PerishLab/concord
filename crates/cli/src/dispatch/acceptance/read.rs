use concord_core::{Coordinate, Result};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;

use super::super::forge::Request;
use super::model::{Connection, Entry, Envelope, Header, Issue, Label, Snapshot};

const LIMIT: usize = 8 * 1024 * 1024;
const QUERY: &str = "query($owner:String!,$name:String!,$number:Int!,$labels:String,$comments:String){repository(owner:$owner,name:$name){id issue(number:$number){id number url state body updated:updatedAt kind:issueType{name} labels(first:100,after:$labels){total:totalCount nodes{name} page:pageInfo{next:hasNextPage cursor:endCursor}} comments(first:100,after:$comments){total:totalCount nodes{id body url} page:pageInfo{next:hasNextPage cursor:endCursor}}}}}";

pub(super) struct Reader<'a> {
    pub coordinate: &'a Coordinate,
    pub command: &'a Path,
    pub timeout: u64,
    pub pages: usize,
}

struct Scan<T> {
    items: Vec<T>,
    cursor: Option<String>,
    total: Option<usize>,
    complete: bool,
    seen: BTreeSet<String>,
}

impl<T> Default for Scan<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            cursor: None,
            total: None,
            complete: false,
            seen: BTreeSet::new(),
        }
    }
}

impl Reader<'_> {
    pub async fn complete(&self) -> Result<Snapshot> {
        let mut header = None;
        let mut labels = Scan::<Label>::default();
        let mut comments = Scan::<Entry>::default();
        for _ in 0..self.pages {
            let (current, issue) = self.page(&labels.cursor, &comments.cursor).await?;
            if header.as_ref().is_some_and(|header| header != &current) {
                return Err(super::fault(
                    "changed",
                    "Issue changed between acceptance pages",
                ));
            }
            header = Some(current);
            labels.append(issue.labels)?;
            comments.append(issue.comments)?;
            bounded(&labels.items, &comments.items)?;
            if labels.complete && comments.complete {
                return snapshot(header.unwrap(), labels.items, comments.items);
            }
        }
        Err(super::fault(
            "truncated",
            "acceptance history exceeds the selected page bound; no absence or readiness is inferred",
        ))
    }

    async fn page(
        &self,
        labels: &Option<String>,
        comments: &Option<String>,
    ) -> Result<(Header, Issue)> {
        let input = serde_json::to_vec(&json!({
            "query": QUERY,
            "variables": {
                "owner": self.coordinate.owner,
                "name": self.coordinate.repository,
                "number": self.coordinate.number,
                "labels": labels,
                "comments": comments,
            },
        }))
        .map_err(|error| super::fault("encode", error.to_string()))?;
        let reply = Request::new(self.command, self.timeout, LIMIT)
            .args(["api", "graphql", "--input", "-"])
            .input(&input)
            .run()
            .await
            .map_err(|error| super::fault("provider", error.to_string()))?;
        let envelope: Envelope = serde_json::from_slice(&reply.stdout).map_err(|error| {
            super::fault(
                "reply",
                format!("malformed acceptance observation: {error}"),
            )
        })?;
        if envelope.errors.is_some() {
            return Err(super::fault(
                "provider",
                "GraphQL acceptance observation contains provider errors",
            ));
        }
        let repository = envelope
            .data
            .and_then(|data| data.repository)
            .ok_or_else(|| super::fault("missing", "requested repository is unreadable"))?;
        let issue = repository
            .issue
            .ok_or_else(|| super::fault("missing", "requested typed Issue is unreadable"))?;
        let header = self.header(repository.id, &issue)?;
        Ok((header, issue))
    }

    fn header(&self, repository: String, issue: &Issue) -> Result<Header> {
        let url = format!(
            "https://github.com/{}/{}/issues/{}",
            self.coordinate.owner, self.coordinate.repository, self.coordinate.number
        );
        let kind = issue
            .kind
            .as_ref()
            .map(|kind| kind.name.clone())
            .unwrap_or_default();
        if issue.number != self.coordinate.number || issue.url != url {
            return Err(super::fault(
                "identity",
                "acceptance observation disagrees with the typed Issue coordinate or identity",
            ));
        }
        for identity in [&issue.id, &repository, &kind, &issue.updated] {
            if identity.trim().is_empty() {
                return Err(super::fault(
                    "identity",
                    "acceptance observation has missing typed Issue metadata",
                ));
            }
        }
        if !matches!(issue.state.as_str(), "OPEN" | "CLOSED") {
            return Err(super::fault(
                "identity",
                "acceptance observation has an unknown Issue state",
            ));
        }
        if issue.body.len() > 65_536 {
            return Err(super::fault(
                "bounds",
                "Issue body exceeds the acceptance observation bound",
            ));
        }
        Ok(Header {
            node: issue.id.clone(),
            repository,
            coordinate: self.coordinate.clone(),
            state: issue.state.clone(),
            kind,
            body: issue.body.clone(),
            updated: issue.updated.clone(),
        })
    }
}

impl<T> Scan<T> {
    fn append(&mut self, connection: Connection<T>) -> Result<()> {
        if self.total.is_some_and(|total| total != connection.total) {
            return Err(super::fault(
                "changed",
                "acceptance connection total changed between pages",
            ));
        }
        self.total = Some(connection.total);
        if self.complete {
            return Ok(());
        }
        if connection.nodes.len() > 100 {
            return Err(super::fault(
                "reply",
                "acceptance page exceeds its requested size",
            ));
        }
        self.items.extend(connection.nodes);
        if self.items.len() > connection.total
            || connection.page.next == (self.items.len() == connection.total)
        {
            return Err(super::fault(
                "reply",
                "acceptance page completeness disagrees with its total",
            ));
        }
        if !connection.page.next {
            self.complete = true;
            return Ok(());
        }
        let cursor = connection
            .page
            .cursor
            .filter(|cursor| !cursor.trim().is_empty())
            .ok_or_else(|| {
                super::fault(
                    "cursor",
                    "incomplete acceptance page has no continuation cursor",
                )
            })?;
        if !self.seen.insert(cursor.clone()) {
            return Err(super::fault(
                "cursor",
                "acceptance pagination repeated a cursor",
            ));
        }
        self.cursor = Some(cursor);
        Ok(())
    }
}

fn bounded(labels: &[Label], entries: &[Entry]) -> Result<()> {
    let mut bytes: usize = labels.iter().map(|label| label.name.len()).sum();
    if bytes > LIMIT {
        return Err(super::fault(
            "bounds",
            "complete acceptance labels exceed the retained byte bound",
        ));
    }
    for entry in entries {
        bytes += entry.body.len() + entry.node.len() + entry.url.len();
        if entry.body.len() > 65_536 || bytes > LIMIT {
            return Err(super::fault(
                "bounds",
                "complete acceptance history exceeds the retained byte bound",
            ));
        }
    }
    Ok(())
}

fn snapshot(header: Header, labels: Vec<Label>, entries: Vec<Entry>) -> Result<Snapshot> {
    let mut names = BTreeSet::new();
    for label in labels {
        if label.name.trim().is_empty() || !names.insert(label.name) {
            return Err(super::fault(
                "reply",
                "missing or repeated acceptance label name",
            ));
        }
    }
    let prefix = format!(
        "https://github.com/{}/{}/issues/{}#issuecomment-",
        header.coordinate.owner, header.coordinate.repository, header.coordinate.number
    );
    let mut nodes = BTreeSet::new();
    let mut urls = BTreeSet::new();
    for entry in &entries {
        let suffix = entry.url.strip_prefix(&prefix).unwrap_or_default();
        if entry.node.trim().is_empty() || !nodes.insert(&entry.node) || !urls.insert(&entry.url) {
            return Err(super::fault(
                "identity",
                "acceptance comment has a missing, repeated or foreign identity",
            ));
        }
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(super::fault(
                "identity",
                "acceptance comment belongs to another Issue or has no provider URL identity",
            ));
        }
    }
    Ok(Snapshot {
        header,
        labels: names.into_iter().collect(),
        entries,
    })
}
