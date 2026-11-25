mod native;

use concord_core::Result;
use concord_core::acceptance::{Fact, Reference, Verdict, digest};
use serde_json::Value;
use std::collections::BTreeSet;

use super::clock::timestamp;
use super::model::{Connection, Snapshot};
use super::read::Scan;
use crate::args::acceptance::Observe;

pub(super) struct Reader<'a> {
    pub args: &'a Observe,
    pub snapshot: &'a Snapshot,
    pub declaration: &'a Reference,
    pub closure: &'a Reference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Stamp {
    id: String,
    created: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Comment {
    id: String,
    body: String,
    created: String,
    edited: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Freshness {
    edited: Option<String>,
    declaration: Comment,
    closure: Comment,
    events: Vec<Stamp>,
}

struct Data {
    repository: Repository,
    declaration: Comment,
    closure: Comment,
}

struct Repository {
    id: String,
    issue: Issue,
}

struct Issue {
    id: String,
    body: String,
    state: String,
    updated: String,
    edited: Value,
    events: Connection<Stamp>,
}

impl Reader<'_> {
    pub async fn complete(&self) -> Result<Freshness> {
        let mut events = Scan::default();
        let mut held = None;
        for _ in 0..self.args.pages {
            let data = self.page(&events.cursor).await?;
            let issue = data.repository.issue;
            let current = Freshness {
                edited: optional(&issue.edited)?,
                declaration: data.declaration,
                closure: data.closure,
                events: Vec::new(),
            };
            if held.as_ref().is_some_and(|previous| previous != &current) {
                return Err(super::fault(
                    "changed",
                    "review metadata changed between pages",
                ));
            }
            held = Some(current);
            events.append(issue.events)?;
            let bytes: usize = events
                .items
                .iter()
                .map(|event| event.id.len() + event.created.len())
                .sum();
            if bytes > 8 * 1024 * 1024 {
                return Err(super::fault(
                    "bounds",
                    "reopened event history exceeds its retained byte bound",
                ));
            }
            if events.complete {
                let mut fresh = held.unwrap();
                fresh.events = events.items;
                fresh.validate()?;
                return Ok(fresh);
            }
        }
        Err(super::fault(
            "truncated",
            "reopened event history exceeds the selected page bound",
        ))
    }

    async fn page(&self, cursor: &Option<String>) -> Result<Data> {
        let data = self.native(cursor)?;
        self.check(&data)?;
        Ok(data)
    }

    fn check(&self, data: &Data) -> Result<()> {
        let header = &self.snapshot.header;
        let issue = &data.repository.issue;
        let expected = [
            &header.repository,
            &header.node,
            &header.body,
            &header.state,
            &header.updated,
        ];
        let observed = [
            &data.repository.id,
            &issue.id,
            &issue.body,
            &issue.state,
            &issue.updated,
        ];
        if observed != expected {
            return Err(super::fault(
                "changed",
                "review metadata disagrees with the observed Issue",
            ));
        }
        for (comment, reference) in [
            (&data.declaration, self.declaration),
            (&data.closure, self.closure),
        ] {
            if comment.id != reference.node || digest(&comment.body) != reference.digest {
                return Err(super::fault(
                    "identity",
                    "review comment identity or body disagrees with current history",
                ));
            }
        }
        Ok(())
    }
}

impl Freshness {
    fn validate(&self) -> Result<()> {
        for comment in [&self.declaration, &self.closure] {
            timestamp(&comment.created)?;
            optional(&comment.edited)?;
        }
        let mut seen = BTreeSet::new();
        for event in &self.events {
            timestamp(&event.created)?;
            if event.id.trim().is_empty() || !seen.insert(&event.id) {
                return Err(super::fault(
                    "identity",
                    "missing or repeated reopened event identity",
                ));
            }
        }
        Ok(())
    }

    pub fn fact(&self) -> Result<Fact> {
        if !self.closure.edited.is_null() {
            return Ok(fact(
                Verdict::Unmet,
                "closure comment was edited; publish a fresh review",
            ));
        }
        let created = self.closure.created.as_str();
        let declaration = optional(&self.declaration.edited)?;
        let changes = self
            .edited
            .iter()
            .chain(declaration.iter())
            .map(String::as_str)
            .chain(self.events.iter().map(|event| event.created.as_str()));
        let mut equal = false;
        for change in changes {
            if change > created {
                return Ok(fact(
                    Verdict::Unmet,
                    "body edit, declaration edit or reopen followed the closure review",
                ));
            }
            equal |= change == created;
        }
        if equal {
            return Ok(fact(
                Verdict::Unknown,
                "provider timestamps cannot order a change and review within the same second",
            ));
        }
        if self.declaration.created.as_str() >= created {
            return Ok(fact(
                Verdict::Unknown,
                "provider timestamps do not establish declaration before review",
            ));
        }
        Ok(fact(
            Verdict::Satisfied,
            "complete current edit and reopen facts precede the unchanged closure review",
        ))
    }
}

fn optional(value: &Value) -> Result<Option<String>> {
    if value.is_null() {
        return Ok(None);
    }
    let text = value
        .as_str()
        .ok_or_else(|| super::fault("reply", "invalid provider edit timestamp"))?;
    timestamp(text)?;
    Ok(Some(text.into()))
}

fn fact(verdict: Verdict, reason: &str) -> Fact {
    Fact {
        verdict,
        reason: reason.into(),
    }
}
