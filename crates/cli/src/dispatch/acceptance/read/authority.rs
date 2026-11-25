use super::super::model::Header;
use crate::args::acceptance::Observe;
use crate::runtime::github;
use concord_core::Result;

#[derive(Debug, Eq, PartialEq)]
pub(in crate::dispatch::acceptance) struct Blob {
    pub oid: String,
    pub text: String,
}

pub(in crate::dispatch::acceptance) async fn manifest(
    args: &Observe,
    header: &Header,
) -> Result<Option<Blob>> {
    let client =
        github::client(args.timeout).map_err(|error| super::super::fault("provider", error))?;
    let found = github::call("pull.manifest", || {
        client.manifest(&header.coordinate.owner, &header.coordinate.repository)
    })
    .map_err(|error| super::super::fault("provider", github::describe(error)))?
    .value;
    if found.repository != header.repository {
        return Err(super::super::fault(
            "identity",
            "release authority repository identity differs",
        ));
    }
    match (found.oid, found.text) {
        (None, None) => Ok(None),
        (Some(oid), Some(text)) if !oid.is_empty() => Ok(Some(Blob { oid, text })),
        _ => Err(super::super::fault(
            "identity",
            "release manifest has no object identity",
        )),
    }
}
