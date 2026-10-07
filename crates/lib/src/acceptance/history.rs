use std::collections::BTreeSet;

use serde::Serialize;

use super::{Marker, Reference, Target};
use crate::Result;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comment {
    pub node: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Record {
    pub reference: Reference,
    pub marker: Marker,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct History {
    pub declaration: Option<Record>,
    pub closure: Option<Record>,
}

impl History {
    pub fn read(comments: &[Comment]) -> Result<Self> {
        let mut history = Self::default();
        let mut seen = BTreeSet::new();
        for comment in comments {
            if comment.node.trim().is_empty() || !seen.insert(&comment.node) {
                return Err(super::fault(
                    "missing or repeated provider comment identity",
                ));
            }
            if let Some(marker) = Marker::parse(&comment.body)? {
                history.apply(Record {
                    reference: Reference {
                        node: comment.node.clone(),
                        digest: super::digest(&comment.body),
                    },
                    marker,
                })?;
            }
        }
        Ok(history)
    }

    pub fn target(&self, labels: &[String]) -> Result<Option<Target>> {
        let label = Target::select(labels)?;
        let declared = self
            .declaration
            .as_ref()
            .map(|record| record.marker.target());
        if label != declared {
            return Err(super::fault(
                "managed target label and current declaration disagree",
            ));
        }
        Ok(label)
    }

    fn apply(&mut self, record: Record) -> Result<()> {
        match &record.marker {
            Marker::Declaration { .. } if self.declaration.is_some() => {
                return Err(super::fault(
                    "a second root declaration requires an explicit amendment",
                ));
            }
            Marker::Declaration { .. } => self.closure = None,
            Marker::Amendment { predecessor, .. } => {
                self.current(predecessor)?;
                self.closure = None;
            }
            Marker::Closure {
                declaration,
                target,
                ..
            } => {
                self.current(declaration)?;
                if self.declaration.as_ref().unwrap().marker.target() != *target {
                    return Err(super::fault("closure target differs from its declaration"));
                }
                self.closure = Some(record);
                return Ok(());
            }
        }
        self.declaration = Some(record);
        Ok(())
    }

    fn current(&self, reference: &Reference) -> Result<()> {
        if self.declaration.as_ref().map(|record| &record.reference) == Some(reference) {
            return Ok(());
        }
        Err(super::fault(
            "acceptance reference is absent, stale or edited",
        ))
    }
}
