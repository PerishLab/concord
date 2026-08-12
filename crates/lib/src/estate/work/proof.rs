use super::super::World;
use super::{Estate, Proof, Worktree, active, stale};
use crate::{Error, Result, git};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proving {
    pub task: String,
    pub member: String,
    pub revision: i64,
}

impl Estate {
    pub async fn prove(&self, proving: &Proving) -> Result<Worktree> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&proving.task)?;
        active(task.life, &proving.task)?;
        stale(task.revision, proving.revision)?;
        let member = self.member(&proving.task, &proving.member).await?;
        let source = self.source(&member)?;
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
            write: &member.claims,
        })
        .map_err(|error| Error::typed("concord.boundary.refused", error.to_string()))?;
        if !report.ok {
            return Err(Error::typed(
                "concord.boundary.outside",
                format!("changes outside claim: {}", report.outside.join(", ")),
            ));
        }
        let proof = Proof {
            key: 0,
            schema: plumb::boundary::SCHEMA.to_string(),
            plumb: crate::PLUMB.to_string(),
            base,
            head,
            claim: crate::claim::digest(&member.claims),
        };
        self.keep(task.key, member.key, task.revision + 1, &proof)
            .await?;
        self.member(&proving.task, &proving.member).await
    }

    async fn keep(&self, task: i64, member: i64, revision: i64, proof: &Proof) -> Result<()> {
        let held = self
            .core
            .live("Boundary")
            .await
            .map_err(super::fault)?
            .into_iter()
            .find(|row| row.int("member") == Some(member));
        let root = member.to_string();
        let next = revision.to_string();
        self.core
            .batch(async |tx| {
                if let Some(held) = &held {
                    tx.end("Boundary", held.key()).await?;
                }
                tx.put(
                    "Boundary",
                    &[
                        ("schema", proof.schema.as_str()),
                        ("plumb", proof.plumb.as_str()),
                        ("base", proof.base.as_str()),
                        ("head", proof.head.as_str()),
                        ("claim", proof.claim.as_str()),
                        ("member", root.as_str()),
                    ],
                )
                .await?;
                tx.set("Task", task, &[("revision", next.as_str())]).await?;
                Ok(())
            })
            .await
            .map_err(super::fault)
    }
}
