use super::super::Evidence;
use crate::{Coordinate, Current, Phase};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SCHEMA: &str = "concord.v0.13-disposition-plan/v1";
pub const PREFLIGHT: &str = "concord.v0.13-disposition-preflight/v1";
pub const ARCHIVE: &str = "concord.v0.13-retired-history/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IssueDestination {
    pub node: String,
    #[serde(flatten)]
    pub coordinate: Coordinate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Disposition {
    Issue {
        #[serde(flatten)]
        issue: IssueDestination,
    },
    ArchiveOnly,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDisposition {
    pub task: String,
    pub revision: i64,
    pub disposition: Disposition,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceDisposition {
    pub task: String,
    pub name: String,
    pub node: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispositionPlan {
    pub schema: String,
    pub inventory: String,
    pub tasks: Vec<TaskDisposition>,
    pub members: Vec<ResourceDisposition>,
    pub artifacts: Vec<ResourceDisposition>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation {
    pub node: String,
    #[serde(flatten)]
    pub coordinate: Coordinate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArchiveTask {
    pub current: Current,
    pub phases: Vec<Phase>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Archive {
    pub schema: String,
    pub source: String,
    pub inventory: String,
    pub database: Evidence,
    pub tasks: Vec<ArchiveTask>,
    pub bytes: u64,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Mapping {
    pub task: String,
    pub name: String,
    pub node: String,
    pub source: PathBuf,
    pub target: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Capacity {
    pub filesystem: PathBuf,
    pub required: u64,
    pub available: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Preflight {
    pub schema: String,
    pub source: String,
    pub inventory: String,
    pub database: Evidence,
    pub sudo: Evidence,
    pub tasks: Vec<TaskDisposition>,
    pub issues: Vec<Observation>,
    pub members: Vec<Mapping>,
    pub artifacts: Vec<Mapping>,
    pub archive: Archive,
    pub capacity: Capacity,
}
