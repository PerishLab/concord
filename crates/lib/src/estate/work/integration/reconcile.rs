use super::{Integration, Rename, decode, node};
use crate::estate::{Estate, fault};
use crate::{Error, Result};

impl Estate {
    pub async fn rename(&self, request: &Rename) -> Result<Integration> {
        node(&request.node)?;
        request.repository.validate()?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let integrations = self.core.live("Integration").await.map_err(fault)?;
        let row = integrations
            .iter()
            .find(|row| row.text("node") == Some(&request.node))
            .ok_or_else(|| {
                Error::typed(
                    "concord.integration.absent",
                    format!("repository node is not registered: {}", request.node),
                )
            })?;
        let held = decode(row)?;
        if held.repository == request.repository {
            return Ok(held);
        }
        if integrations.iter().any(|other| {
            other.key() != row.key()
                && other.text("owner") == Some(&request.repository.owner)
                && other.text("repository") == Some(&request.repository.name)
        }) {
            return Err(Error::typed(
                "concord.integration.coordinate_reserved",
                format!(
                    "repository coordinate is already registered: {}",
                    request.repository.identity()
                ),
            ));
        }
        let members = self.core.live("IssueMember").await.map_err(fault)?;
        let anchors = self.core.live("Anchor").await.map_err(fault)?;
        let moving = members
            .iter()
            .filter(|member| member.int("integration") == Some(row.key()))
            .filter_map(|member| member.int("anchor"))
            .collect::<Vec<_>>();
        reserve(&anchors, &moving, request)?;
        self.core
            .batch(async |tx| {
                tx.set(
                    "Integration",
                    row.key(),
                    &[
                        ("owner", request.repository.owner.as_str()),
                        ("repository", request.repository.name.as_str()),
                    ],
                )
                .await?;
                for anchor in anchors
                    .iter()
                    .filter(|anchor| moving.contains(&anchor.key()))
                {
                    let revision = anchor.int("revision").unwrap_or_default() + 1;
                    let revision = revision.to_string();
                    tx.set(
                        "Anchor",
                        anchor.key(),
                        &[
                            ("owner", request.repository.owner.as_str()),
                            ("repository", request.repository.name.as_str()),
                            ("revision", revision.as_str()),
                        ],
                    )
                    .await?;
                }
                Ok(())
            })
            .await
            .map_err(fault)?;
        self.locate(&request.node).await
    }
}

fn reserve(anchors: &[keel::Row], moving: &[i64], request: &Rename) -> Result<()> {
    for anchor in anchors
        .iter()
        .filter(|anchor| moving.contains(&anchor.key()))
    {
        let number = anchor
            .int("number")
            .ok_or_else(|| Error::typed("concord.issue.row", "Anchor has no number"))?;
        let wanted = (
            Some(request.repository.owner.as_str()),
            Some(request.repository.name.as_str()),
            Some(number),
        );
        if anchors.iter().any(|other| {
            !moving.contains(&other.key())
                && (
                    other.text("owner"),
                    other.text("repository"),
                    other.int("number"),
                ) == wanted
        }) {
            return Err(Error::typed(
                "concord.issue.coordinate_reserved",
                format!(
                    "Issue coordinate is already attached: {}#{number}",
                    request.repository.identity()
                ),
            ));
        }
    }
    Ok(())
}
