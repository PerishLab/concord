use super::super::World;
use super::{Estate, MemberChange, active, stale};
use crate::{Error, Result, git};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Narrowing {
    pub task: String,
    pub member: String,
    pub claims: Vec<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn narrow(&self, narrowing: &Narrowing) -> Result<MemberChange> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&narrowing.task)?;
        active(task.life, &narrowing.task)?;
        stale(task.revision, narrowing.revision)?;
        let member = self.member(&task.identity(), &narrowing.member).await?;
        let claims = crate::claim::normalize(&narrowing.claims)?;
        let source = self.source(&member)?;
        let observations = self.overlaps(Some(member.key), &source, &claims).await?;
        if claims == member.claims {
            return Err(Error::typed(
                "concord.claim.unchanged",
                "claim narrowing does not change the normalized claim",
            ));
        }
        let path = self.path(&task.domain, &task.name, &member.name);
        if !git::at(&source).clean()? {
            return Err(Error::typed(
                "concord.member.source",
                "integration checkout is not clean",
            ));
        }
        let head = git::at(&path).head()?;
        let origin = git::at(&source).head()?;
        let base = git::at(&path).merge(&head, &origin)?;
        let report = plumb::boundary::check(plumb::boundary::Request {
            root: &path,
            base: &base,
            head: &head,
            write: &claims,
        })
        .map_err(|error| Error::typed("concord.boundary.refused", error.to_string()))?;
        if !report.ok {
            return Err(Error::typed(
                "concord.boundary.outside",
                format!("changes outside claim: {}", report.outside.join(", ")),
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
            member: self.member(&task.identity(), &narrowing.member).await?,
            observations,
        })
    }
}
