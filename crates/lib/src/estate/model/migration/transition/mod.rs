mod collect;
mod source;

use self::source::{agree, evidence, exact, fingerprint, settled};
use super::super::super::{
    Estate, Finding, Life, Origin, Proof, Realm, Reference, Repository, Seat, Weight,
};
use crate::{Error, Result};
use serde::Serialize;
use std::path::PathBuf;

#[cfg(test)]
mod tests;

pub const SCHEMA: &str = "concord.v0.13-transition-inventory/v1";
pub const SOURCE: &str = "v0.13.0";
pub const TARGET: &str = "issue-execution";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Evidence {
    pub path: PathBuf,
    pub bytes: u64,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Task {
    pub key: i64,
    pub identity: String,
    pub life: Life,
    pub revision: i64,
    pub facts: usize,
    pub phases: usize,
    pub phase_entries: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<Reference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dependency {
    pub key: i64,
    pub source: String,
    pub target: String,
    pub weight: Weight,
    pub origin: Origin,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Member {
    pub key: i64,
    pub task: String,
    pub name: String,
    pub source: String,
    pub source_path: PathBuf,
    pub source_identity: PathBuf,
    pub branch: String,
    pub path: PathBuf,
    pub head: String,
    pub claims: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary: Option<Proof>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<Reference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Artifact {
    pub task: String,
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub digest: String,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Counts {
    pub domains: usize,
    pub active_tasks: usize,
    pub retired_tasks: usize,
    pub facts: usize,
    pub phases: usize,
    pub phase_entries: usize,
    pub dependencies: usize,
    pub members: usize,
    pub claims: usize,
    pub boundaries: usize,
    pub artifacts: usize,
    pub filesystem_seats: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Inventory {
    pub schema: String,
    pub source: String,
    pub target: String,
    pub fingerprint: String,
    pub database: Evidence,
    pub sudo: Evidence,
    pub graph_revision: i64,
    pub domains: Vec<Realm>,
    pub repositories: Vec<Repository>,
    pub tasks: Vec<Task>,
    pub dependencies: Vec<Dependency>,
    pub members: Vec<Member>,
    pub artifacts: Vec<Artifact>,
    pub counts: Counts,
    pub observations: Vec<Finding>,
}

pub struct Transition<'a> {
    seat: &'a Seat,
}

impl Seat {
    pub fn transition(&self) -> Transition<'_> {
        Transition { seat: self }
    }
}

impl Transition<'_> {
    pub async fn inventory(&self) -> Result<Inventory> {
        settled(&self.seat.database())?;
        let database = evidence(&self.seat.database())?;
        let sudo = evidence(&self.seat.sudo())?;
        let possession = std::fs::read_to_string(self.seat.sudo())?;
        let core = self
            .seat
            .bind(super::super::released, &possession)
            .await
            .map_err(|error| {
                Error::typed(
                    "concord.transition.source",
                    format!("estate is not the exact {SOURCE} source model: {error}"),
                )
            })?;
        let estate = Estate {
            core,
            space: self.seat.space.clone(),
            anchors: false,
            execution: false,
        };
        let agreement = estate.inspect(None, None).await?;
        agree(&agreement)?;
        let mut inventory =
            collect::read(&estate, agreement, database.clone(), sudo.clone()).await?;
        drop(estate);
        settled(&self.seat.database())?;
        exact("database", &database, &evidence(&self.seat.database())?)?;
        exact("sudo", &sudo, &evidence(&self.seat.sudo())?)?;
        inventory.fingerprint = fingerprint(&inventory)?;
        Ok(inventory)
    }
}
