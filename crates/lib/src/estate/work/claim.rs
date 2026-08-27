use super::super::World;
use super::{Estate, MemberChange, active, stale};
use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claiming {
    pub task: String,
    pub member: String,
    pub claims: Vec<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn claim(&self, claiming: &Claiming) -> Result<MemberChange> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&claiming.task)?;
        active(task.life, &claiming.task)?;
        stale(task.revision, claiming.revision)?;
        let member = self.member(&task.identity(), &claiming.member).await?;
        let mut union = member.claims.clone();
        union.extend(claiming.claims.clone());
        let claims = crate::claim::normalize(&union)?;
        let source = self.source(&member)?;
        let observations = self.overlaps(Some(member.key), &source, &claims).await?;
        if claims == member.claims {
            return Err(Error::typed(
                "concord.claim.unchanged",
                "claim expansion does not change the normalized claim",
            ));
        }
        let rows = self.core.live("Claim").await.map_err(super::fault)?;
        let boundary = self
            .core
            .live("Boundary")
            .await
            .map_err(super::fault)?
            .into_iter()
            .find(|row| row.int("member") == Some(member.key));
        let revision = task.revision + 1;
        let next = revision.to_string();
        let parent = member.key.to_string();
        self.core
            .batch(async |tx| {
                for row in rows
                    .iter()
                    .filter(|row| row.int("member") == Some(member.key))
                {
                    tx.end("Claim", row.key()).await?;
                }
                if let Some(boundary) = &boundary {
                    tx.end("Boundary", boundary.key()).await?;
                }
                for claim in &claims {
                    tx.put(
                        "Claim",
                        &[("path", claim.as_str()), ("member", parent.as_str())],
                    )
                    .await?;
                }
                tx.set("Task", task.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(super::fault)?;
        Ok(MemberChange {
            member: self.member(&task.identity(), &claiming.member).await?,
            observations,
        })
    }
}
