use crate::args::acceptance::Observe;
use crate::runtime::github;
use concord_core::acceptance::Target;
use concord_core::{Coordinate, Result};

const COLOR: &str = "5319e7";

pub(super) async fn resolve(
    args: &Observe,
    repository: &str,
    target: Target,
    create: bool,
) -> Result<String> {
    let coordinate = Coordinate::parse(&args.issue)?;
    let client = github::client(args.timeout).map_err(|error| super::fault("provider", error))?;
    let found = github::call("issue.badge", || {
        client.badge(&coordinate.owner, &coordinate.repository, target.label())
    })
    .map_err(|error| super::fault("provider", github::describe(error)))?
    .value;
    if found.repository != repository {
        return Err(super::fault("reply", "label repository identity differs"));
    }
    if let Some(badge) = found.badge {
        return Ok(badge.id);
    }
    if !create {
        return Err(super::fault(
            "missing",
            "managed predecessor label is unreadable",
        ));
    }
    let minted = github::call("issue.mint", || {
        client.mint(repository, target.label(), COLOR)
    })
    .map_err(|error| super::fault("provider", github::describe(error)))?
    .value;
    Ok(minted.id)
}

pub(super) async fn alter(args: &Observe, issue: &str, label: &str, remove: bool) -> Result<()> {
    let client = github::client(args.timeout).map_err(|error| super::fault("provider", error))?;
    github::call("issue.relabel", || client.relabel(issue, label, remove))
        .map_err(|error| super::fault("provider", github::describe(error)))?;
    Ok(())
}
