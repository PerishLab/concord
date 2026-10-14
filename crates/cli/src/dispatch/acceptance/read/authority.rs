use concord_core::Result;
use serde::Deserialize;
use serde_json::json;

use super::super::model::Header;
use crate::args::acceptance::Observe;
use crate::dispatch::forge::Request;

const LIMIT: usize = 8 * 1024 * 1024;
const QUERY: &str = "query($owner:String!,$name:String!){repository(owner:$owner,name:$name){id object(expression:\"HEAD:plumb.toml\"){... on Blob {oid text}}}}";

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct Envelope {
    data: Option<Data>,
    errors: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct Data {
    repository: Option<Repository>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct Repository {
    id: String,
    object: Option<Blob>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub(in crate::dispatch::acceptance) struct Blob {
    pub oid: String,
    pub text: String,
}

pub(in crate::dispatch::acceptance) async fn manifest(
    args: &Observe,
    header: &Header,
) -> Result<Option<Blob>> {
    let input = serde_json::to_vec(&json!({"query": QUERY, "variables": {
        "owner": header.coordinate.owner, "name": header.coordinate.repository,
    }}))
    .map_err(|error| super::super::fault("encode", error.to_string()))?;
    let reply = Request::new(&args.command, args.timeout, LIMIT)
        .args(["api", "graphql", "--input", "-"])
        .input(&input)
        .run()
        .await
        .map_err(|error| super::super::fault("provider", error.to_string()))?;
    let envelope: Envelope = serde_json::from_slice(&reply.stdout)
        .map_err(|error| super::super::fault("reply", error.to_string()))?;
    if envelope.errors.is_some() {
        return Err(super::super::fault(
            "provider",
            "release authority query has provider errors",
        ));
    }
    let repository = envelope
        .data
        .and_then(|data| data.repository)
        .ok_or_else(|| super::super::fault("missing", "release repository is unreadable"))?;
    if repository.id != header.repository {
        return Err(super::super::fault(
            "identity",
            "release authority repository identity differs",
        ));
    }
    if repository
        .object
        .as_ref()
        .is_some_and(|blob| blob.oid.is_empty())
    {
        return Err(super::super::fault(
            "identity",
            "release manifest has no object identity",
        ));
    }
    Ok(repository.object)
}
