use concord_core::Result;
use concord_core::acceptance::{Fact, History, Marker, Release, Target, Verdict, Verification};
use plumb::seat::release::{Authority, Distribution};
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
struct Blob {
    oid: String,
    text: String,
}

pub(super) async fn read(args: &Observe, header: &Header, history: &History) -> Result<Fact> {
    if history
        .declaration
        .as_ref()
        .is_none_or(|record| record.marker.target() != Target::Release)
    {
        return Ok(fact(Verdict::Unknown, "no release target to evaluate"));
    }
    let Some(Marker::Closure {
        release: Some(release),
        verification,
        ..
    }) = history.closure.as_ref().map(|record| &record.marker)
    else {
        return Ok(fact(Verdict::Unknown, "no current release review"));
    };
    let first = manifest(args, header).await?;
    let authority = first
        .as_ref()
        .map(|blob| Authority::declared(&blob.text))
        .transpose()
        .map_err(|error| super::super::fault("declaration", error))?
        .flatten();
    let Some(authority) = authority else {
        return Ok(fact(
            Verdict::Unknown,
            "repository declares no release authority",
        ));
    };
    let Some(marker) = authority.record(&release.distribution) else {
        return Ok(fact(
            Verdict::Unmet,
            "distribution is outside the repository release authority",
        ));
    };
    if marker.channel != "stable" || marker.marker != release.marker {
        return Ok(fact(
            Verdict::Unmet,
            "release review does not name one exact stable distribution",
        ));
    }
    let observed = tokio::task::spawn_blocking(move || {
        let first = authority.read(&marker);
        let second = authority.read(&marker);
        if first != second {
            return Err("distribution changed during release observation".into());
        }
        second
    })
    .await
    .map_err(|error| super::super::fault("provider", error.to_string()))?;
    if manifest(args, header).await? != first {
        return Err(super::super::fault(
            "changed",
            "release authority changed during evaluation",
        ));
    }
    Ok(assess(release, *verification, observed))
}

async fn manifest(args: &Observe, header: &Header) -> Result<Option<Blob>> {
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

fn assess(
    release: &Release,
    verification: Verification,
    observed: std::result::Result<Option<Distribution>, String>,
) -> Fact {
    let distribution = match observed {
        Ok(Some(distribution)) => distribution,
        Ok(None) => return fact(Verdict::Unknown, "exact distribution record is absent"),
        Err(error) => {
            return fact(
                Verdict::Unknown,
                format!("distribution observation failed: {error}"),
            );
        }
    };
    if distribution.marker != release.marker {
        return fact(
            Verdict::Unmet,
            "distribution marker differs from release review",
        );
    }
    if !distribution.complete() {
        return fact(Verdict::Unmet, "exact stable distribution is not complete");
    }
    if distribution.commit.len() != 40
        || !distribution
            .commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || distribution.public().is_empty()
    {
        return fact(
            Verdict::Unknown,
            "complete distribution lacks a source commit or public media",
        );
    }
    if release.inclusion.is_empty() || verification != Verification::Manual {
        return fact(
            Verdict::Unknown,
            "distribution alone does not prove inclusion; machine inclusion evidence is not available",
        );
    }
    let record = json!({"marker": distribution.marker, "commit": distribution.commit,
        "state": distribution.state, "media": distribution.media, "run": distribution.run});
    let digest = concord_core::acceptance::digest(&record.to_string());
    fact(
        Verdict::Satisfied,
        format!(
            "exact stable {} distribution is complete at {} (record {digest}); inclusion remains explicitly manual review, not automated ancestry proof",
            distribution.marker, distribution.commit,
        ),
    )
}

fn fact(verdict: Verdict, reason: impl Into<String>) -> Fact {
    Fact {
        verdict,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> Release {
        Release {
            marker: "v1.0.0".into(),
            distribution:
                "https://releases.example.org/v1/releases/stable/v1.0.0/distribution.json".into(),
            inclusion: vec!["reviewed merge ancestry and installed behavior".into()],
        }
    }

    fn distribution() -> Distribution {
        Distribution {
            marker: "v1.0.0".into(),
            commit: "a".repeat(40),
            state: "complete".into(),
            media: [("binary".into(), "published".into())].into(),
            run: None,
        }
    }

    #[test]
    fn manual() {
        let held = assess(&release(), Verification::Manual, Ok(Some(distribution())));
        assert_eq!(held.verdict, Verdict::Satisfied);
        assert!(held.reason.contains("explicitly manual"));
        let mut changed = distribution();
        changed.run = Some("another-run".into());
        assert_ne!(
            held,
            assess(&release(), Verification::Manual, Ok(Some(changed)))
        );
    }

    #[test]
    fn machine() {
        assert_eq!(
            assess(&release(), Verification::Machine, Ok(Some(distribution()))).verdict,
            Verdict::Unknown
        );
    }

    #[test]
    fn unavailable() {
        for observed in [Ok(None), Err("offline".into()), Err("changed".into())] {
            assert_eq!(
                assess(&release(), Verification::Manual, observed).verdict,
                Verdict::Unknown
            );
        }
    }

    #[test]
    fn incomplete() {
        let mut held = distribution();
        held.state = "pending".into();
        assert_eq!(
            assess(&release(), Verification::Manual, Ok(Some(held))).verdict,
            Verdict::Unmet
        );
    }

    #[test]
    fn conflicting() {
        let mut held = distribution();
        held.marker = "v2.0.0".into();
        assert_eq!(
            assess(&release(), Verification::Manual, Ok(Some(held))).verdict,
            Verdict::Unmet
        );
    }

    #[test]
    fn unsupported() {
        for held in [
            Distribution {
                commit: String::new(),
                ..distribution()
            },
            Distribution {
                media: Default::default(),
                ..distribution()
            },
        ] {
            assert_eq!(
                assess(&release(), Verification::Manual, Ok(Some(held))).verdict,
                Verdict::Unknown
            );
        }
        let mut reviewed = release();
        reviewed.inclusion.clear();
        assert_eq!(
            assess(&reviewed, Verification::Manual, Ok(Some(distribution()))).verdict,
            Verdict::Unknown
        );
    }
}
