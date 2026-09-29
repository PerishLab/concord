use super::super::{Estate, fault};
use crate::{Error, Result, component};
use keel::Row;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Coordinate {
    pub owner: String,
    pub repository: String,
    pub number: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub key: i64,
    pub node: String,
    #[serde(flatten)]
    pub coordinate: Coordinate,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Admission {
    pub node: String,
    pub coordinate: Coordinate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reconcile {
    pub anchor: Coordinate,
    pub node: String,
    pub repository: String,
    pub coordinate: Coordinate,
    pub revision: i64,
}

impl Estate {
    pub async fn issues(&self) -> Result<Vec<Anchor>> {
        let mut anchors = self
            .core
            .live("Anchor")
            .await
            .map_err(fault)?
            .iter()
            .map(decode)
            .collect::<Result<Vec<_>>>()?;
        anchors.sort_by_key(|anchor| anchor.key);
        Ok(anchors)
    }

    pub async fn admit(&self, request: &Admission) -> Result<Anchor> {
        node(&request.node)?;
        request.coordinate.validate()?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchors = self.core.live("Anchor").await.map_err(fault)?;
        if anchors
            .iter()
            .any(|row| row.text("node") == Some(&request.node))
        {
            return Err(Error::typed(
                "concord.issue.node_reserved",
                format!("Issue node is already attached: {}", request.node),
            ));
        }
        if anchors.iter().any(|row| same(row, &request.coordinate)) {
            return Err(Error::typed(
                "concord.issue.coordinate_reserved",
                format!(
                    "Issue coordinate is already attached: {}",
                    request.coordinate.identity()
                ),
            ));
        }
        let number = request.coordinate.number.to_string();
        let key = self
            .core
            .put(
                "Anchor",
                &[
                    ("node", request.node.as_str()),
                    ("owner", request.coordinate.owner.as_str()),
                    ("repository", request.coordinate.repository.as_str()),
                    ("number", number.as_str()),
                    ("revision", "0"),
                ],
            )
            .await
            .map_err(fault)?;
        Ok(Anchor {
            key,
            node: request.node.clone(),
            coordinate: request.coordinate.clone(),
            revision: 0,
        })
    }

    pub async fn issue(&self, coordinate: &Coordinate) -> Result<Anchor> {
        coordinate.validate()?;
        let anchors = self.core.live("Anchor").await.map_err(fault)?;
        anchors
            .iter()
            .find(|row| same(row, coordinate))
            .map(decode)
            .transpose()?
            .ok_or_else(|| {
                Error::typed(
                    "concord.issue.absent",
                    format!("Issue anchor not found: {}", coordinate.identity()),
                )
            })
    }

    pub async fn reconcile(&self, request: &Reconcile) -> Result<Anchor> {
        node(&request.node)?;
        node(&request.repository)?;
        request.anchor.validate()?;
        request.coordinate.validate()?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchors = self.core.live("Anchor").await.map_err(fault)?;
        let row = anchors
            .iter()
            .find(|row| same(row, &request.anchor))
            .ok_or_else(|| {
                Error::typed(
                    "concord.issue.absent",
                    format!("Issue anchor not found: {}", request.anchor.identity()),
                )
            })?;
        let held = decode(row)?;
        if held.revision != request.revision {
            return Err(Error::typed(
                "concord.issue.stale",
                format!(
                    "Issue execution revision changed: expected {}, found {}",
                    request.revision, held.revision
                ),
            ));
        }
        if held.node != request.node {
            return Err(Error::typed(
                "concord.issue.node_mismatch",
                format!(
                    "Issue node changed: expected {}, found {}",
                    held.node, request.node
                ),
            ));
        }
        if anchors
            .iter()
            .any(|other| other.key() != row.key() && same(other, &request.coordinate))
        {
            return Err(Error::typed(
                "concord.issue.coordinate_reserved",
                format!(
                    "Issue coordinate is already attached: {}",
                    request.coordinate.identity()
                ),
            ));
        }
        let member = self
            .issue_worktrees()
            .await?
            .into_iter()
            .find(|member| member.node == held.node);
        if let Some(member) = &member
            && member.integration.node != request.repository
        {
            return Err(Error::typed(
                "concord.issue.repository_transfer",
                "Issue cannot transfer to a different repository while its Member exists",
            ));
        }
        if let Some(member) = &member
            && (member.integration.repository.owner != request.coordinate.owner
                || member.integration.repository.name != request.coordinate.repository)
        {
            return Err(Error::typed(
                "concord.integration.coordinate",
                "reconcile the stable repository Integration before its Issue coordinates",
            ));
        }
        let revision = held.revision + 1;
        let number = request.coordinate.number.to_string();
        let next = revision.to_string();
        self.core
            .batch(async |tx| {
                tx.set(
                    "Anchor",
                    row.key(),
                    &[
                        ("owner", request.coordinate.owner.as_str()),
                        ("repository", request.coordinate.repository.as_str()),
                        ("number", number.as_str()),
                        ("revision", next.as_str()),
                    ],
                )
                .await?;
                Ok(())
            })
            .await
            .map_err(fault)?;
        Ok(Anchor {
            key: held.key,
            node: held.node,
            coordinate: request.coordinate.clone(),
            revision,
        })
    }
}

fn same(row: &Row, coordinate: &Coordinate) -> bool {
    row.text("owner") == Some(&coordinate.owner)
        && row.text("repository") == Some(&coordinate.repository)
        && row.int("number") == Some(coordinate.number)
}

pub(in crate::estate) fn decode(row: &Row) -> Result<Anchor> {
    let text = |field| {
        row.text(field)
            .map(str::to_string)
            .ok_or_else(|| malformed(row.key(), field))
    };
    let coordinate = Coordinate {
        owner: text("owner")?,
        repository: text("repository")?,
        number: row
            .int("number")
            .ok_or_else(|| malformed(row.key(), "number"))?,
    };
    coordinate.validate()?;
    let identity = text("node")?;
    node(&identity)?;
    let revision = row
        .int("revision")
        .filter(|revision| *revision >= 0)
        .ok_or_else(|| malformed(row.key(), "revision"))?;
    Ok(Anchor {
        key: row.key(),
        node: identity,
        coordinate,
        revision,
    })
}

fn node(node: &str) -> Result<()> {
    component("issue node", node)?;
    if node.chars().any(char::is_whitespace) {
        return Err(Error::typed(
            "concord.issue.node",
            "Issue node cannot contain whitespace",
        ));
    }
    Ok(())
}

fn malformed(key: i64, field: &str) -> Error {
    Error::typed(
        "concord.issue.row",
        format!("Issue Anchor {key} has malformed field {field}"),
    )
}
