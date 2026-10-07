use serde::{Deserialize, Serialize};

use super::{Comment, History, Marker, Reference, Target};
use crate::Result;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    marker: Marker,
    body: String,
    basis: Vec<Reference>,
    prior: Option<Target>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    Publish,
    Relabel,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Recovery {
    pub step: Step,
    pub reference: Option<Reference>,
    pub target: Target,
}

impl Plan {
    pub fn prepare(
        marker: Marker,
        body: String,
        labels: &[String],
        comments: &[Comment],
    ) -> Result<Self> {
        let prior = History::read(comments)?.target(labels)?;
        let plan = Self {
            marker,
            body,
            basis: references(comments)?,
            prior,
        };
        if plan.recover(labels, comments)?.step != Step::Publish {
            return Err(super::fault(
                "acceptance intent already exists in the observed history",
            ));
        }
        Ok(plan)
    }

    pub fn body(&self) -> Result<&str> {
        if Marker::parse(&self.body)?.as_ref() != Some(&self.marker) {
            return Err(super::fault(
                "planned body differs from its acceptance marker",
            ));
        }
        Ok(&self.body)
    }

    pub fn recover(&self, labels: &[String], comments: &[Comment]) -> Result<Recovery> {
        self.body()?;
        History::read(comments)?;
        let label = Target::select(labels)?;
        let target = self.marker.target();
        let found: Vec<_> = comments
            .iter()
            .filter(|comment| comment.body == self.body)
            .collect();
        let removed = label.is_none() && found.len() == 1;
        if !removed && label != self.prior && label != Some(target) {
            return Err(super::fault(
                "managed target changed outside the acceptance plan",
            ));
        }
        match found.as_slice() {
            [] => self.pending(comments),
            [comment] => self.retained(label, comments, comment),
            _ => Err(super::fault(
                "intended acceptance comment has multiple identities",
            )),
        }
    }

    fn pending(&self, comments: &[Comment]) -> Result<Recovery> {
        self.basis(comments)?;
        self.append(comments)?;
        Ok(Recovery {
            step: Step::Publish,
            reference: None,
            target: self.marker.target(),
        })
    }

    fn retained(
        &self,
        label: Option<Target>,
        comments: &[Comment],
        intended: &Comment,
    ) -> Result<Recovery> {
        let before: Vec<_> = comments
            .iter()
            .filter(|comment| comment.node != intended.node)
            .cloned()
            .collect();
        self.basis(&before)?;
        self.append(&before)?;
        let target = self.marker.target();
        Ok(Recovery {
            step: if label == Some(target) {
                Step::Complete
            } else {
                Step::Relabel
            },
            reference: Some(reference(intended)),
            target,
        })
    }

    fn basis(&self, comments: &[Comment]) -> Result<()> {
        if references(comments)? != self.basis {
            return Err(super::fault("acceptance history changed outside the plan"));
        }
        let target = History::read(comments)?
            .declaration
            .map(|record| record.marker.target());
        if target != self.prior {
            return Err(super::fault(
                "planned predecessor target disagrees with history",
            ));
        }
        Ok(())
    }

    fn append(&self, comments: &[Comment]) -> Result<()> {
        let node = format!("planned:{}", super::digest(&self.body));
        let mut proposed = comments.to_vec();
        proposed.push(Comment {
            node,
            body: self.body.clone(),
        });
        History::read(&proposed)?;
        Ok(())
    }
}

fn references(comments: &[Comment]) -> Result<Vec<Reference>> {
    let mut references = Vec::new();
    for comment in comments {
        if Marker::parse(&comment.body)?.is_some() {
            references.push(reference(comment));
        }
    }
    Ok(references)
}

fn reference(comment: &Comment) -> Reference {
    Reference {
        node: comment.node.clone(),
        digest: super::digest(&comment.body),
    }
}
