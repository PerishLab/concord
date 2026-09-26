use super::super::{Node, Reference};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Goal,
    Constraint,
    Decision,
    Focus,
    Question,
    Next,
    Addition,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Fact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<i64>,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Edit {
    Create { fact: Fact },
    Set { fact: Fact },
    End { role: Role, key: i64 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Patch {
    pub version: u32,
    pub task: String,
    pub revision: i64,
    pub edits: Vec<Edit>,
}

impl Patch {
    pub const SHAPE: &'static str = r#"{
  "version": 1,
  "task": "DOMAIN/NAME",
  "revision": 0,
  "edits": [
    { "op": "create", "fact": { "role": "goal", "body": "TEXT" } },
    { "op": "set", "fact": { "key": 1, "role": "focus", "body": "TEXT" } },
    { "op": "end", "role": "next", "key": 1 }
  ]
}"#;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Current {
    pub task: Node,
    pub facts: Vec<Fact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<Reference>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Part {
    Outcome,
    Decision,
    Evidence,
    Carry,
    Addition,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<i64>,
    pub part: Part,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Settle {
    pub version: u32,
    pub task: String,
    pub revision: i64,
    pub phase: Vec<Entry>,
    #[serde(default)]
    pub edits: Vec<Edit>,
}

impl Settle {
    pub const SHAPE: &'static str = r#"{
  "version": 1,
  "task": "DOMAIN/NAME",
  "revision": 0,
  "phase": [
    { "part": "outcome", "body": "TEXT" },
    { "part": "decision", "title": "TEXT", "body": "TEXT" }
  ],
  "edits": [
    { "op": "set", "fact": { "key": 1, "role": "focus", "body": "TEXT" } }
  ]
}"#;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Phase {
    pub key: i64,
    pub number: i64,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Settlement {
    pub current: Current,
    pub phase: Phase,
}

impl Role {
    pub const ALL: [Self; 7] = [
        Self::Goal,
        Self::Constraint,
        Self::Decision,
        Self::Focus,
        Self::Question,
        Self::Next,
        Self::Addition,
    ];

    pub(in crate::estate) fn unit(self) -> &'static str {
        match self {
            Self::Goal => "task:goal",
            Self::Constraint => "task:constraint",
            Self::Decision => "task:decision",
            Self::Focus => "task:focus",
            Self::Question => "task:question",
            Self::Next => "task:next",
            Self::Addition => "task:addition",
        }
    }

    pub(in crate::estate) fn singular(self) -> bool {
        matches!(self, Self::Goal | Self::Focus | Self::Next)
    }
}

impl Part {
    pub const ALL: [Self; 5] = [
        Self::Outcome,
        Self::Decision,
        Self::Evidence,
        Self::Carry,
        Self::Addition,
    ];

    pub(in crate::estate) fn unit(self) -> &'static str {
        match self {
            Self::Outcome => "phase:outcome",
            Self::Decision => "phase:decision",
            Self::Evidence => "phase:evidence",
            Self::Carry => "phase:carry",
            Self::Addition => "phase:addition",
        }
    }

    pub(in crate::estate) fn singular(self) -> bool {
        self == Self::Outcome
    }
}
