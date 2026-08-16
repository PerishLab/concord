use super::super::World;
use super::{Estate, active, stale};
use crate::{Error, Result, git};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    pub task: String,
    pub member: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Retirement {
    pub task: String,
    pub member: String,
    pub artifacts: Vec<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn release(&self, release: &Release) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&release.task)?;
        active(task.life, &release.task)?;
        stale(task.revision, release.revision)?;
        let member = self.member(&task.identity(), &release.member).await?;
        let proof = member.proof.as_ref().ok_or_else(|| {
            Error::typed(
                "concord.boundary.absent",
                "Member has no current Boundary proof",
            )
        })?;
        let path = self.path(&task.domain, &task.name, &member.name);
        let source = self.source(&member)?;
        let head = git::at(&path).head()?;
        let held = (
            proof.schema.as_str(),
            proof.plumb.as_str(),
            proof.head.as_str(),
            proof.claim.as_str(),
        );
        let claim = crate::claim::digest(&member.claims);
        let current = (
            plumb::boundary::SCHEMA,
            crate::PLUMB,
            head.as_str(),
            claim.as_str(),
        );
        if held != current {
            return Err(Error::typed(
                "concord.boundary.stale",
                "Member Boundary proof is stale",
            ));
        }
        if !git::at(&path).clean()? {
            return Err(Error::typed(
                "concord.member.dirty",
                "Member has dirty or untracked files",
            ));
        }
        if !git::at(&path).landed(&source)? {
            return Err(Error::typed(
                "concord.member.unlanded",
                "Member HEAD is neither reachable nor tree-equivalent to source HEAD",
            ));
        }
        let claims = self.core.live("Claim").await.map_err(super::fault)?;
        let revision = task.revision + 1;
        let next = revision.to_string();
        git::at(&source).remove(&path)?;
        let changed = self
            .core
            .batch(async |tx| {
                tx.end("Boundary", proof.key).await?;
                for claim in claims
                    .iter()
                    .filter(|claim| claim.int("member") == Some(member.key))
                {
                    tx.end("Claim", claim.key()).await?;
                }
                tx.end("Member", member.key).await?;
                tx.set("Task", task.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await;
        if let Err(error) = changed {
            return Err(Error::typed(
                "concord.member.disagreement",
                format!("{error}; worktree is removed but estate still declares it"),
            ));
        }
        if let Some(root) = path.parent() {
            let _ = std::fs::remove_dir(root);
        }
        Ok(revision)
    }

    pub async fn retire(&self, retirement: &Retirement) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&retirement.task)?;
        active(task.life, &retirement.task)?;
        stale(task.revision, retirement.revision)?;
        let member = self.member(&task.identity(), &retirement.member).await?;
        let proof = member.proof.as_ref().ok_or_else(|| {
            Error::typed(
                "concord.boundary.absent",
                "Member has no current Boundary proof",
            )
        })?;
        let path = self.path(&task.domain, &task.name, &member.name);
        let source = self.source(&member)?;
        let head = git::at(&path).head()?;
        let held = (
            proof.schema.as_str(),
            proof.plumb.as_str(),
            proof.head.as_str(),
            proof.claim.as_str(),
        );
        let claim = crate::claim::digest(&member.claims);
        let current = (
            plumb::boundary::SCHEMA,
            crate::PLUMB,
            head.as_str(),
            claim.as_str(),
        );
        if held != current {
            return Err(Error::typed(
                "concord.boundary.stale",
                "Member Boundary proof is stale",
            ));
        }
        if !git::at(&path).clean()? {
            return Err(Error::typed(
                "concord.member.dirty",
                "Member has dirty or untracked files",
            ));
        }
        let identity = task.identity();
        let artifacts = self.artifacts(&identity).await?;
        if !artifacts.iter().any(|artifact| {
            retirement
                .artifacts
                .iter()
                .any(|pattern| pattern == "*" || pattern == &artifact.name)
        }) {
            return Err(Error::typed(
                "concord.artifact.absent",
                "no Artifact matches the given patterns",
            ));
        }
        let claims = self.core.live("Claim").await.map_err(super::fault)?;
        let revision = task.revision + 1;
        let next = revision.to_string();
        git::at(&source).remove(&path)?;
        let changed = self
            .core
            .batch(async |tx| {
                tx.end("Boundary", proof.key).await?;
                for claim in claims
                    .iter()
                    .filter(|claim| claim.int("member") == Some(member.key))
                {
                    tx.end("Claim", claim.key()).await?;
                }
                tx.end("Member", member.key).await?;
                tx.set("Task", task.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await;
        if let Err(error) = changed {
            return Err(Error::typed(
                "concord.member.disagreement",
                format!("{error}; worktree is removed but estate still declares it"),
            ));
        }
        if let Some(root) = path.parent() {
            let _ = std::fs::remove_dir(root);
        }
        Ok(revision)
    }
}
