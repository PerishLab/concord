use super::super::{Estate, Life, Node, fault};
use crate::{Error, Result};
use keel::Row;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;

const CATALOGUE: usize = 64;

pub(in crate::estate) struct World {
    pub space: i64,
    pub revision: i64,
    pub domains: BTreeMap<i64, Realm>,
    pub nodes: Vec<Node>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Realm {
    pub key: i64,
    pub name: String,
    pub revision: i64,
}

impl World {
    #[locus::trace(with = crate::observation::view())]
    pub async fn load(estate: &Estate) -> Result<Self> {
        let spaces = estate.core.live("Space").await.map_err(fault)?;
        let domains = estate.core.live("Domain").await.map_err(fault)?;
        let tasks = estate.core.live("Task").await.map_err(fault)?;
        Self::read(&spaces, &domains, &tasks)
    }

    pub fn read(spaces: &[Row], domains: &[Row], tasks: &[Row]) -> Result<Self> {
        if spaces.len() != 1 {
            return Err(Error::typed(
                "concord.estate.space",
                format!("Concord estate needs one Space, found {}", spaces.len()),
            ));
        }
        let space = spaces[0].key();
        let revision = integer(&spaces[0], "revision")?;
        let mut held = BTreeMap::new();
        for row in domains {
            if integer(row, "space")? != space {
                return Err(Error::typed(
                    "concord.domain.space",
                    format!("Domain {} has a foreign Space", row.key()),
                ));
            }
            held.insert(
                row.key(),
                Realm {
                    key: row.key(),
                    name: text(row, "name")?.to_string(),
                    revision: integer(row, "revision")?,
                },
            );
        }
        if held.values().any(|realm| realm.revision < 0) {
            return Err(Error::typed(
                "concord.domain.revision",
                "Domain revision cannot be negative",
            ));
        }
        let mut nodes = Vec::with_capacity(tasks.len());
        for row in tasks {
            let domain = integer(row, "domain")?;
            let name = held.get(&domain).ok_or_else(|| {
                Error::typed(
                    "concord.task.domain",
                    format!("Task {} has an unknown Domain", row.key()),
                )
            })?;
            nodes.push(Node {
                key: row.key(),
                domain: name.name.clone(),
                name: text(row, "name")?.to_string(),
                life: Life::parse(text(row, "state")?)?,
                revision: integer(row, "revision")?,
            });
        }
        nodes.sort_by_key(Node::identity);
        Ok(Self {
            space,
            revision,
            domains: held,
            nodes,
        })
    }

    pub fn domain(&self, name: &str) -> Option<i64> {
        self.domains
            .iter()
            .find_map(|(key, held)| (held.name == name).then_some(*key))
    }

    pub fn node(&self, identity: &str) -> Result<&Node> {
        if let Some((domain, name)) = identity.split_once('/') {
            return self
                .nodes
                .iter()
                .find(|node| node.domain == domain && node.name == name)
                .ok_or_else(|| self.missing(identity, Some(domain)));
        }
        let found = self
            .nodes
            .iter()
            .filter(|node| node.name == identity)
            .collect::<Vec<_>>();
        match found.as_slice() {
            [] => Err(self.missing(identity, None)),
            [node] => Ok(node),
            _ => Err(Error::detailed(
                "concord.task.ambiguous",
                format!("task name is ambiguous; use domain/{identity}"),
                json!({"identity": identity, "tasks": qualified(&found)}),
            )),
        }
    }

    pub fn absent(&self, domain: &str) -> Error {
        Error::detailed(
            "concord.domain.absent",
            format!("unknown managed domain {domain}"),
            json!({"domain": domain, "domains": self.realms()}),
        )
    }

    fn missing(&self, identity: &str, domain: Option<&str>) -> Error {
        let held: Vec<&Node> = match domain {
            Some(domain) if self.domain(domain).is_none() => {
                return Error::detailed(
                    "concord.task.absent",
                    format!("task not found: {identity}"),
                    json!({"identity": identity, "domains": self.realms()}),
                );
            }
            Some(domain) => self.nodes.iter().filter(|n| n.domain == domain).collect(),
            None => self.nodes.iter().collect(),
        };
        let live: Vec<&Node> = held
            .iter()
            .copied()
            .filter(|node| node.life == Life::Active)
            .collect();
        Error::detailed(
            "concord.task.absent",
            format!("task not found: {identity}"),
            json!({
                "identity": identity,
                "tasks": qualified(&live),
                "active": live.len(),
                "retired": held.len() - live.len(),
            }),
        )
    }

    fn realms(&self) -> Vec<&str> {
        self.domains
            .values()
            .map(|held| held.name.as_str())
            .collect()
    }
}

fn qualified(held: &[&Node]) -> Vec<String> {
    held.iter()
        .take(CATALOGUE)
        .map(|node| format!("{}/{}", node.domain, node.name))
        .collect()
}

fn text<'a>(row: &'a Row, field: &str) -> Result<&'a str> {
    row.text(field).ok_or_else(|| malformed(row, field))
}

fn integer(row: &Row, field: &str) -> Result<i64> {
    row.int(field).ok_or_else(|| malformed(row, field))
}

fn malformed(row: &Row, field: &str) -> Error {
    Error::typed(
        "concord.estate.row",
        format!("Resource {} has malformed field {field}", row.key()),
    )
}
