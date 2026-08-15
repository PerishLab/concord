use super::World;
use super::{Estate, Finish, Life, Node, Origin, Realm, Weight, fault};
use crate::component;
use crate::{Error, Result};
use keel::Tie;

#[path = "data/coordinate.rs"]
mod coordinate;
#[path = "data/domain.rs"]
mod domain;

pub use coordinate::{Rehome, Rename};
pub use domain::{Annotate, Repository};

impl Estate {
    pub async fn realms(&self) -> Result<Vec<Realm>> {
        let world = World::load(self).await?;
        let mut realms = world.domains.into_values().collect::<Vec<_>>();
        realms.sort_by_key(|realm| (realm.name.clone(), realm.key));
        Ok(realms)
    }

    pub async fn manage(&self, name: &str) -> Result<i64> {
        component("domain name", name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        if world.domain(name).is_some() {
            return Err(Error::typed(
                "concord.domain.reserved",
                format!("Domain name is already reserved: {name}"),
            ));
        }
        self.core
            .put(
                "Domain",
                &[
                    ("name", name),
                    ("revision", "0"),
                    ("space", &world.space.to_string()),
                ],
            )
            .await
            .map_err(fault)
    }

    pub async fn start(&self, domain: &str, name: &str) -> Result<Node> {
        component("domain name", domain)?;
        component("task name", name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let root = world.domain(domain).ok_or_else(|| world.absent(domain))?;
        let identity = format!("{domain}/{name}");
        if reserved(self, root, name).await? {
            return Err(Error::typed(
                "concord.task.reserved",
                format!("Task name is already reserved: {identity}"),
            ));
        }
        let path = self.space.join(domain).join(".tasks").join(name);
        if path.exists() {
            return Err(Error::typed(
                "concord.task.territory",
                format!(
                    "Task seat is occupied by foreign territory: {}",
                    path.display()
                ),
            ));
        }
        let root = root.to_string();
        let key = self
            .core
            .batch(async |tx| {
                let key = tx
                    .put(
                        "Task",
                        &[
                            ("name", name),
                            ("state", "active"),
                            ("revision", "0"),
                            ("domain", root.as_str()),
                        ],
                    )
                    .await?;
                tx.put("Reservation", &[("name", name), ("domain", root.as_str())])
                    .await?;
                Ok(key)
            })
            .await
            .map_err(fault)?;
        Ok(Node {
            key,
            domain: domain.to_string(),
            name: name.to_string(),
            life: Life::Active,
            revision: 0,
        })
    }

    pub async fn node(&self, identity: &str) -> Result<Node> {
        World::load(self).await?.node(identity).cloned()
    }

    pub async fn nodes(&self, retired: bool) -> Result<Vec<Node>> {
        let world = World::load(self).await?;
        Ok(world
            .nodes
            .into_iter()
            .filter(|node| retired || node.life == Life::Active)
            .collect())
    }

    pub async fn finish(&self, finish: &Finish) -> Result<Node> {
        if finish.reason.trim().is_empty() {
            return Err(Error::typed(
                "concord.task.reason",
                "Task retirement reason cannot be blank",
            ));
        }
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&finish.task)?;
        if task.life != Life::Active {
            return Err(Error::typed(
                "concord.task.retired",
                format!("Task is already retired: {}", finish.task),
            ));
        }
        if task.revision != finish.revision || world.revision != finish.graph {
            return Err(Error::typed(
                "concord.task.stale",
                format!(
                    "finish revisions changed: Task {}/{}, graph {}/{}",
                    finish.revision, task.revision, finish.graph, world.revision
                ),
            ));
        }
        let members = self.core.live("Member").await.map_err(fault)?;
        if members
            .iter()
            .any(|member| member.int("task") == Some(task.key))
        {
            return Err(Error::typed(
                "concord.task.members",
                "Task finish requires zero live Members",
            ));
        }
        self.finishable(task)?;
        let incident = self.incident(&world, task.key).await?;
        let revision = task.revision + 1;
        let graph = world.revision + 1;
        let next = revision.to_string();
        let step = graph.to_string();
        let space = world.space.to_string();
        self.core
            .batch(async |tx| {
                for tie in &incident {
                    archive(tx, tie, &space, finish.reason.as_str()).await?;
                    tx.cut("Task", "depends", tie.key()).await?;
                }
                tx.set(
                    "Task",
                    task.key,
                    &[("state", "retired"), ("revision", next.as_str())],
                )
                .await?;
                tx.set("Space", world.space, &[("revision", step.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(fault)?;
        let mut retired = task.clone();
        retired.life = Life::Retired;
        retired.revision = revision;
        Ok(retired)
    }

    fn finishable(&self, task: &Node) -> Result<()> {
        let seat = self
            .space
            .join(&task.domain)
            .join(".tasks")
            .join(&task.name);
        if !seat.exists() {
            return Ok(());
        }
        let memory = seat.join(".task");
        for entry in std::fs::read_dir(&seat)? {
            let path = entry?.path();
            if path != memory {
                return Err(territory(&path));
            }
        }
        if !memory.exists() {
            return Ok(());
        }
        if !memory.is_dir() {
            return Err(territory(&memory));
        }
        let artifacts = memory.join("artifacts");
        for entry in std::fs::read_dir(&memory)? {
            let path = entry?.path();
            if path != artifacts {
                return Err(territory(&path));
            }
        }
        if artifacts.exists() && !artifacts.is_dir() {
            return Err(territory(&artifacts));
        }
        if artifacts.is_dir() && std::fs::read_dir(&artifacts)?.next().is_some() {
            return Err(Error::typed(
                "concord.task.artifacts",
                "Task finish requires zero retained Artifacts",
            ));
        }
        Ok(())
    }

    async fn incident(&self, world: &World, task: i64) -> Result<Vec<Tie>> {
        let mut ties = Vec::new();
        for node in &world.nodes {
            for tie in self
                .core
                .ties("Task", "depends", node.key)
                .await
                .map_err(fault)?
            {
                if tie.left() == task || tie.right() == task {
                    ties.push(tie);
                }
            }
        }
        ties.sort_by_key(Tie::key);
        ties.dedup_by_key(|tie| tie.key());
        Ok(ties)
    }
}

pub(super) async fn reserved(estate: &Estate, domain: i64, name: &str) -> Result<bool> {
    Ok(estate
        .core
        .live("Reservation")
        .await
        .map_err(fault)?
        .iter()
        .any(|row| row.int("domain") == Some(domain) && row.text("name") == Some(name)))
}

fn territory(path: &std::path::Path) -> Error {
    Error::typed(
        "concord.task.territory",
        format!("Task seat contains foreign territory: {}", path.display()),
    )
}

async fn archive(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    tie: &Tie,
    space: &str,
    reason: &str,
) -> std::result::Result<(), keel::adapt::Error> {
    let source = tie.left().to_string();
    let target = tie.right().to_string();
    let weight = Weight::parse(tie.text("weight").unwrap_or(""))
        .map_err(|error| keel::adapt::Error::Adapt(error.to_string()))?;
    let origin = Origin::parse(tie.text("origin").unwrap_or(""))
        .map_err(|error| keel::adapt::Error::Adapt(error.to_string()))?;
    tx.put(
        "Retirement",
        &[
            ("weight", weight.name()),
            ("origin", origin.name()),
            ("reason", reason),
            ("space", space),
            ("source", source.as_str()),
            ("target", target.as_str()),
        ],
    )
    .await?;
    Ok(())
}
