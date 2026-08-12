use super::World;
use super::{Cut, Edge, Estate, Life, Link, Node, Origin, Tune, Weight, fault};
use crate::{Error, Result};
use keel::{Ends, Tie};

#[path = "data/target.rs"]
mod target;
use target::Target;

impl Estate {
    pub async fn depend(&self, link: &Link) -> Result<i64> {
        self.declare(link, false).await
    }

    pub async fn declare(&self, link: &Link, create: bool) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        stale(&world, link.revision)?;
        let source = world.node(&link.source)?;
        active(source.life, &link.source)?;
        let target = match world.node(&link.target) {
            Ok(target) => {
                active(target.life, &link.target)?;
                Some(target)
            }
            Err(error) if create && error.code() == "concord.task.absent" => None,
            Err(error) => return Err(error),
        };
        if target.is_some_and(|target| source.key == target.key) {
            return Err(Error::typed(
                "concord.dependency.self",
                "a Task cannot depend on itself",
            ));
        }
        if let Some(target) = target
            && self
                .core
                .ties("Task", "depends", source.key)
                .await
                .map_err(fault)?
                .iter()
                .any(|edge| edge.right() == target.key)
        {
            return Err(Error::typed(
                "concord.dependency.exists",
                format!(
                    "dependency already exists: {} -> {}",
                    link.source, link.target
                ),
            ));
        }
        let revision = world.revision + 1;
        let from = source.key;
        let target = match target {
            Some(target) => Target::Held(target.key),
            None => Target::Fresh(target::fresh(self, &world, &link.target).await?),
        };
        let made = self
            .core
            .batch(async |tx| {
                let to = match &target {
                    Target::Held(key) => *key,
                    Target::Fresh((domain, name)) => {
                        let domain = domain.to_string();
                        let key = tx
                            .put(
                                "Task",
                                &[
                                    ("name", name.as_str()),
                                    ("state", "active"),
                                    ("revision", "0"),
                                    ("domain", domain.as_str()),
                                ],
                            )
                            .await?;
                        tx.put(
                            "Reservation",
                            &[("name", name.as_str()), ("domain", domain.as_str())],
                        )
                        .await?;
                        key
                    }
                };
                tx.tie(
                    "Task",
                    "depends",
                    Ends {
                        left: from,
                        right: to,
                    },
                    &[
                        ("weight", link.weight.name()),
                        ("origin", link.origin.name()),
                    ],
                )
                .await?;
                tx.set(
                    "Space",
                    world.space,
                    &[("revision", revision.to_string().as_str())],
                )
                .await?;
                Ok(())
            })
            .await;
        match made {
            Ok(()) => Ok(revision),
            Err(error) if error.to_string().contains("cycle") => Err(Error::typed(
                "concord.dependency.cycle",
                format!(
                    "dependency creates a cycle: {} -> {}",
                    link.source, link.target
                ),
            )),
            Err(error) => Err(fault(error)),
        }
    }

    pub async fn reach(&self, identity: &str) -> Result<Vec<Node>> {
        let world = World::load(self).await?;
        let source = world.node(identity)?;
        let ties = self
            .core
            .ties("Task", "depends_closure", source.key)
            .await
            .map_err(fault)?;
        let mut nodes = ties
            .iter()
            .filter_map(|edge| world.nodes.iter().find(|node| node.key == edge.right()))
            .cloned()
            .collect::<Vec<_>>();
        nodes.sort_by_key(Node::identity);
        Ok(nodes)
    }

    pub async fn weigh(&self, tune: &Tune) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        stale(&world, tune.revision)?;
        let source = world.node(&tune.source)?;
        let target = world.node(&tune.target)?;
        active(source.life, &tune.source)?;
        active(target.life, &tune.target)?;
        let tie = self.direct(source.key, target.key).await?;
        let revision = world.revision + 1;
        let next = revision.to_string();
        self.core
            .batch(async |tx| {
                tx.tune(
                    "Task",
                    "depends",
                    tie.key(),
                    &[("weight", tune.weight.name())],
                )
                .await?;
                tx.set("Space", world.space, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(fault)?;
        Ok(revision)
    }

    pub async fn detach(&self, cut: &Cut) -> Result<i64> {
        if cut.reason.trim().is_empty() {
            return Err(Error::typed(
                "concord.dependency.reason",
                "dependency retirement reason cannot be blank",
            ));
        }
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        stale(&world, cut.revision)?;
        let source = world.node(&cut.source)?;
        let target = world.node(&cut.target)?;
        active(source.life, &cut.source)?;
        active(target.life, &cut.target)?;
        let tie = self.direct(source.key, target.key).await?;
        let weight = Weight::parse(tie.text("weight").unwrap_or(""))?;
        let origin = Origin::parse(tie.text("origin").unwrap_or(""))?;
        let revision = world.revision + 1;
        let next = revision.to_string();
        let space = world.space.to_string();
        let from = source.key.to_string();
        let to = target.key.to_string();
        self.core
            .batch(async |tx| {
                tx.put(
                    "Retirement",
                    &[
                        ("weight", weight.name()),
                        ("origin", origin.name()),
                        ("reason", cut.reason.as_str()),
                        ("space", space.as_str()),
                        ("source", from.as_str()),
                        ("target", to.as_str()),
                    ],
                )
                .await?;
                tx.cut("Task", "depends", tie.key()).await?;
                tx.set("Space", world.space, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(fault)?;
        Ok(revision)
    }

    pub(super) async fn edges(&self, world: &World) -> Result<Vec<Edge>> {
        let mut edges = Vec::new();
        for node in &world.nodes {
            for tie in self
                .core
                .ties("Task", "depends", node.key)
                .await
                .map_err(fault)?
            {
                edges.push(Edge {
                    key: tie.key(),
                    source: tie.left(),
                    target: tie.right(),
                    weight: Weight::parse(tie.text("weight").unwrap_or(""))?,
                    origin: Origin::parse(tie.text("origin").unwrap_or(""))?,
                });
            }
        }
        edges.sort_by_key(|edge| (edge.source, edge.target, edge.key));
        Ok(edges)
    }

    async fn direct(&self, source: i64, target: i64) -> Result<Tie> {
        self.core
            .ties("Task", "depends", source)
            .await
            .map_err(fault)?
            .into_iter()
            .find(|tie| tie.right() == target)
            .ok_or_else(|| {
                Error::typed(
                    "concord.dependency.absent",
                    format!("dependency does not exist: {source} -> {target}"),
                )
            })
    }
}

fn stale(world: &World, expected: i64) -> Result<()> {
    if world.revision == expected {
        return Ok(());
    }
    Err(Error::typed(
        "concord.graph.stale",
        format!(
            "graph revision changed: expected {expected}, found {}",
            world.revision
        ),
    ))
}

fn active(life: Life, identity: &str) -> Result<()> {
    if life == Life::Active {
        return Ok(());
    }
    Err(Error::typed(
        "concord.task.retired",
        format!("retired Task is immutable: {identity}"),
    ))
}
