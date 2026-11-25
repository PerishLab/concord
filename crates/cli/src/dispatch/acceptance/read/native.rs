use super::super::{fault, model};
use super::{Reader, relations};
use crate::runtime::github;
use concord_core::Result;
use runseal::github::{Client, Page, Target, Ticket, Window};

pub(super) struct Native {
    client: Client,
    target: Target,
}

impl Reader<'_> {
    pub(super) fn native(&self) -> Result<Native> {
        let number = u32::try_from(self.coordinate.number)
            .map_err(|_| fault("identity", "invalid Issue number"))?;
        let target = Target::new(&self.coordinate.owner, &self.coordinate.repository, number)
            .map_err(provider)?;
        let client = github::client(self.timeout).map_err(|error| fault("provider", error))?;
        Ok(Native { client, target })
    }
}

impl Native {
    pub(super) fn acceptance(
        &self,
        cursors: [&Option<String>; 2],
    ) -> Result<(String, model::Issue)> {
        let [labels, comments] = windows(cursors)?;
        let held = github::call("issue.acceptance", || {
            self.client.acceptance(&self.target, &labels, &comments)
        })
        .map_err(provider)?
        .value;
        let Ticket {
            id,
            number,
            url,
            state,
            body,
            updated,
            kind,
            repository,
            ..
        } = held.issue;
        let labels = connection(held.labels, |runseal::github::Label { name }| {
            model::Label { name }
        });
        let comments = connection(held.comments, |entry| model::Entry {
            node: entry.id,
            body: entry.body,
            url: entry.url,
        });
        Ok((
            repository.id,
            model::Issue {
                id,
                number: number.into(),
                url,
                state,
                body,
                updated,
                kind: kind.map(|runseal::github::Label { name }| model::Kind { name }),
                labels,
                comments,
            },
        ))
    }
}

pub(super) fn relationships(
    native: &Native,
    cursors: [&Option<String>; 2],
) -> Result<relations::Repository> {
    let [children, blockers] = windows(cursors)?;
    let held = github::call("issue.dependencies", || {
        native
            .client
            .dependencies(&native.target, &children, &blockers)
    })
    .map_err(provider)?
    .value;
    let Ticket {
        id,
        body,
        state,
        updated,
        repository,
        ..
    } = held.issue;
    let status = |runseal::github::review::Status { id, state }| relations::Node { id, state };
    Ok(relations::Repository {
        id: repository.id,
        issue: relations::Issue {
            id,
            body,
            state,
            updated,
            children: connection(held.children, status),
            blockers: connection(held.blockers, status),
        },
    })
}

fn windows(cursors: [&Option<String>; 2]) -> Result<[Window; 2]> {
    let [first, second] = cursors;
    Ok([
        Window::new(100, first.as_deref()).map_err(provider)?,
        Window::new(100, second.as_deref()).map_err(provider)?,
    ])
}

fn connection<T, U>(page: Page<T>, convert: impl Fn(T) -> U) -> model::Connection<U> {
    model::Connection {
        total: page.total as usize,
        nodes: page.rows.into_iter().map(convert).collect(),
        page: model::Page {
            next: page.next.is_some(),
            cursor: page.next,
        },
    }
}

fn provider(error: runseal::github::Error) -> concord_core::Error {
    fault("provider", github::describe(error))
}
