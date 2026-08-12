use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Weight {
    Unknown,
    Context,
    Sequence,
    Required,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    Declared,
    Legacy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Life {
    Active,
    Retired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Flow {
    Out,
    In,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Degree {
    pub out: usize,
    pub r#in: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Node {
    pub key: i64,
    pub domain: String,
    pub name: String,
    pub life: Life,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Edge {
    pub key: i64,
    pub source: i64,
    pub target: i64,
    pub weight: Weight,
    pub origin: Origin,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Graph {
    pub revision: i64,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub weight: Weight,
    pub origin: Origin,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tune {
    pub source: String,
    pub target: String,
    pub weight: Weight,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cut {
    pub source: String,
    pub target: String,
    pub reason: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finish {
    pub task: String,
    pub revision: i64,
    pub graph: i64,
    pub reason: String,
}

impl Weight {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "unknown" => Ok(Self::Unknown),
            "context" => Ok(Self::Context),
            "sequence" => Ok(Self::Sequence),
            "required" => Ok(Self::Required),
            _ => Err(Error::typed(
                "concord.dependency.weight",
                format!("invalid dependency weight {value}"),
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Context => "context",
            Self::Sequence => "sequence",
            Self::Required => "required",
        }
    }
}

impl Origin {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "declared" => Ok(Self::Declared),
            "legacy-todo" => Ok(Self::Legacy),
            _ => Err(Error::typed(
                "concord.dependency.origin",
                format!("invalid dependency origin {value}"),
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Legacy => "legacy-todo",
        }
    }
}

impl Life {
    pub(in crate::estate) fn parse(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "retired" => Ok(Self::Retired),
            _ => Err(Error::typed(
                "concord.task.life",
                format!("invalid task life {value}"),
            )),
        }
    }
}

impl Node {
    pub fn identity(&self) -> String {
        format!("{}/{}", self.domain, self.name)
    }
}
