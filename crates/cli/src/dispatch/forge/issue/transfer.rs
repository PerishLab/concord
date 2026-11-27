use crate::args::issue::Transfer;
use crate::runtime::github;
use concord_core::{Admission, Coordinate, Error, Estate, Result};
use runseal::github::{Client, Target, Window, records::Observation};
use serde_json::json;
use std::collections::BTreeSet;

const MARKER: &str = "concord.issue-transfer/v1";

pub(super) async fn run(estate: &Estate, request: &Transfer, output: bool) -> Result<()> {
    let source = admission(&request.issue, &request.old)?;
    let destination = admission(&request.destination, &request.node)?;
    let native = Native::new(&source, &destination, request.timeout)?;
    let held = estate.attachment(&source).await?;
    let attached = estate.attachment(&destination).await?;
    let url = native.observe()?;
    let recorded = native.recorded(&url)?;
    if request.recover {
        let status = match (&held, &attached, &recorded) {
            (None, Some(_), Some(_)) => "recorded-and-source-absent",
            (Some(_), _, Some(_)) => "recorded-source-retained",
            (_, _, Some(_)) => "recorded-destination-absent",
            _ => "record-unobserved",
        };
        return super::super::super::emit(
            json!({"status":status,"source":held,"destination":attached,"record":recorded}),
            output,
        );
    }
    let held = held.ok_or_else(|| fault("source Anchor is absent; use --recover to observe"))?;
    if !request.apply {
        estate.qualify(&held, &destination).await?;
        return super::super::super::emit(
            json!({"status":"qualified","source":held,"destination":attached,"record":recorded}),
            output,
        );
    }
    estate
        .transfer(&held, &destination, async || {
            native.observe()?;
            let mut record = native.recorded(&url)?;
            if record.is_none() {
                let reply =
                    super::comment::submit(&destination.node, &native.body, request.timeout)
                        .await?;
                super::comment::verify(&destination.coordinate, &native.body, &reply)?;
                record = native.recorded(&url)?;
                if record.as_deref() != Some(&reply.url) {
                    return Err(fault(
                        "GitHub comment outcome is indeterminate; use --recover",
                    ));
                }
            }
            native.observe()?;
            Ok(())
        })
        .await?;
    let destination = estate.attachment(&destination).await?;
    super::super::super::emit(
        json!({"status":"settled","destination":destination}),
        output,
    )
}

struct Native<'a> {
    client: Client,
    original: Target,
    destination: Target,
    node: &'a str,
    body: String,
}

impl<'a> Native<'a> {
    fn new(source: &Admission, destination: &'a Admission, timeout: u64) -> Result<Self> {
        if source.node == destination.node || source.coordinate == destination.coordinate {
            return Err(fault(
                "transfer requires distinct Issue coordinates and nodes",
            ));
        }
        let old = source.coordinate.identity();
        let new = destination.coordinate.identity();
        let body = format!(
            "Concord qualified a GitHub Issue transfer from {old} (node `{}`) to {new} (node `{}`). This record identifies the unused source Anchor selected for retirement. The current local estate determines whether retirement settled.\n\n<!-- {MARKER} -->",
            source.node, destination.node
        );
        Ok(Self {
            client: github::client(timeout).map_err(fault)?,
            original: target(&source.coordinate)?,
            destination: target(&destination.coordinate)?,
            node: &destination.node,
            body,
        })
    }

    fn observe(&self) -> Result<String> {
        let reply = github::call("issue.locate", || self.client.locate(&self.original))
            .map_err(provider)?;
        match reply.value {
            Observation::Moved { moved }
                if reply.evidence.status == Some(301)
                    && moved.original == self.original
                    && moved.destination == self.destination => {}
            _ => {
                return Err(fault(
                    "GitHub did not redirect the exact source to the exact destination",
                ));
            }
        }
        let issue = github::call("issue.show", || self.client.ticket(&self.destination))
            .map_err(provider)?
            .value;
        if issue.node != self.node {
            return Err(fault(
                "GitHub destination node disagrees with the requested node",
            ));
        }
        match issue.kind.as_ref().map(|kind| kind.name.as_str()) {
            Some("Feature" | "Bug" | "Task") => {}
            _ => return Err(fault("GitHub destination must be an ordinary typed Issue")),
        }
        if issue
            .labels
            .iter()
            .any(|label| label.name.starts_with("needs:"))
        {
            return Err(fault("GitHub destination has an unresolved needs label"));
        }
        Ok(issue.url)
    }

    fn recorded(&self, issue: &str) -> Result<Option<String>> {
        let prefix = format!("{issue}#issuecomment-");
        let mut after = None;
        let mut seen = BTreeSet::new();
        let mut total = None;
        let mut record = None;
        for _ in 0..20 {
            let window = Window::new(50, after.as_deref()).map_err(provider)?;
            let page = github::call("issue.comments", || {
                self.client.comments(&self.destination, &window)
            })
            .map_err(provider)?
            .value;
            if page.target != self.node || total.is_some_and(|total| total != page.total) {
                return Err(fault("GitHub comment history changed during observation"));
            }
            total = Some(page.total);
            for row in page.rows {
                if !seen.insert(row.id) {
                    return Err(fault("GitHub comment history repeated a node"));
                }
                if row.body != self.body {
                    continue;
                }
                if !row.url.starts_with(&prefix) || row.created != row.updated || record.is_some() {
                    return Err(fault("GitHub transfer record is ambiguous or edited"));
                }
                record = Some(row.url);
            }
            after = page.next;
            if after.is_none() && seen.len() == page.total as usize {
                return Ok(record);
            }
            if after.is_none() {
                break;
            }
        }
        Err(fault("GitHub transfer comment history is incomplete"))
    }
}

fn admission(issue: &str, node: &str) -> Result<Admission> {
    if node.is_empty()
        || node.len() > 256
        || !node
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-=".contains(&byte))
    {
        return Err(fault("invalid GitHub Issue node"));
    }
    Ok(Admission {
        coordinate: Coordinate::parse(issue)?,
        node: node.to_string(),
    })
}

fn target(coordinate: &Coordinate) -> Result<Target> {
    let number = u32::try_from(coordinate.number).map_err(|_| fault("invalid Issue number"))?;
    Target::new(&coordinate.owner, &coordinate.repository, number).map_err(provider)
}

fn provider(error: runseal::github::Error) -> Error {
    fault(github::describe(error))
}

fn fault(message: impl Into<String>) -> Error {
    Error::typed("concord.issue.transfer", message)
}
