use super::super::{fault, model::Page};
use super::{Comment, Connection, Data, Issue, Reader, Repository, Stamp};
use crate::runtime::github;
use concord_core::Result;
use runseal::github::{Target, Window, acceptance};
use serde_json::json;

impl Reader<'_> {
    pub(super) fn native(&self, cursor: &Option<String>) -> Result<Data> {
        let client = github::client(self.args.timeout).map_err(|error| fault("provider", error))?;
        let coordinate = &self.snapshot.header.coordinate;
        let number = u32::try_from(coordinate.number)
            .map_err(|_| fault("identity", "invalid Issue number"))?;
        let target =
            Target::new(&coordinate.owner, &coordinate.repository, number).map_err(provider)?;
        let window = Window::new(100, cursor.as_deref()).map_err(provider)?;
        let pair =
            acceptance::Pair::new(&self.declaration.node, &self.closure.node).map_err(provider)?;
        let held = github::call("issue.freshness", || {
            client.freshness(&target, &window, &pair)
        })
        .map_err(provider)?
        .value;
        let acceptance::History { issue, events } = held.history;
        let acceptance::Subject {
            id,
            body,
            state,
            updated,
            edited,
            repository,
            ..
        } = issue;
        Ok(Data {
            repository: Repository {
                id: repository.id,
                issue: Issue {
                    id,
                    body,
                    state,
                    updated,
                    edited: json!(edited),
                    events: history(events),
                },
            },
            declaration: comment(held.declaration),
            closure: comment(held.closure),
        })
    }
}

fn history(held: runseal::github::Page<acceptance::Event>) -> Connection<Stamp> {
    let nodes = held
        .rows
        .into_iter()
        .map(|acceptance::Event { id, created }| Stamp { id, created })
        .collect();
    Connection {
        total: held.total as usize,
        nodes,
        page: Page {
            next: held.next.is_some(),
            cursor: held.next,
        },
    }
}

fn comment(held: acceptance::Comment) -> Comment {
    let acceptance::Comment {
        id,
        body,
        created,
        edited,
        ..
    } = held;
    Comment {
        id,
        body,
        created,
        edited: json!(edited),
    }
}

fn provider(error: runseal::github::Error) -> concord_core::Error {
    fault("provider", github::describe(error))
}
