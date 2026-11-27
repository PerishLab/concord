use super::super::{Estate, Scope, fault};
use super::{Admission, Anchor};
use crate::{Error, Result};
use std::future::Future;
use std::io::ErrorKind;

impl Estate {
    pub async fn transfer<F, R>(
        &self,
        source: &Anchor,
        destination: &Admission,
        record: F,
    ) -> Result<()>
    where
        F: FnOnce() -> R,
        R: Future<Output = Result<()>>,
    {
        let _guard = self.guard()?;
        self.qualify(source, destination).await?;
        record().await?;
        let number = destination.coordinate.number.to_string();
        let attached = self.attachment(destination).await?;
        self.core
            .batch(async |tx| {
                if attached.is_none() {
                    tx.put(
                        "Anchor",
                        &[
                            ("node", destination.node.as_str()),
                            ("owner", destination.coordinate.owner.as_str()),
                            ("repository", destination.coordinate.repository.as_str()),
                            ("number", number.as_str()),
                            ("revision", "0"),
                        ],
                    )
                    .await?;
                }
                tx.end("Anchor", source.key).await?;
                Ok(())
            })
            .await
            .map_err(fault)
    }

    pub async fn qualify(&self, source: &Anchor, destination: &Admission) -> Result<()> {
        self.ensure(Scope::Control).await?;
        if source.coordinate == destination.coordinate || source.node == destination.node {
            return Err(conflict(
                "transfer requires distinct Issue coordinates and nodes",
            ));
        }
        let current = self.issue(&source.coordinate).await?;
        if current != *source || current.revision != 0 {
            return Err(conflict(
                "source must retain its exact unused Anchor at revision zero",
            ));
        }
        for resource in ["IssueMember", "IssueClaim", "IssueBoundary", "IssueChange"] {
            let rows = self.core.live(resource).await.map_err(fault)?;
            if rows.iter().any(|row| row.int("anchor") == Some(source.key)) {
                return Err(conflict("source Anchor retains local execution resources"));
            }
        }
        let root = self.layout.issue(&source.node);
        match std::fs::symlink_metadata(&root) {
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
            Ok(_) => return Err(conflict("source Issue path still exists")),
        }
        self.attachment(destination).await?;
        Ok(())
    }

    pub async fn attachment(&self, destination: &Admission) -> Result<Option<Anchor>> {
        destination.coordinate.validate()?;
        super::anchor::node(&destination.node)?;
        let anchors = self.issues().await?;
        let found = anchors.iter().find(|anchor| {
            anchor.node == destination.node || anchor.coordinate == destination.coordinate
        });
        match found {
            Some(anchor)
                if anchor.node == destination.node
                    && anchor.coordinate == destination.coordinate =>
            {
                Ok(Some(anchor.clone()))
            }
            Some(_) => Err(conflict(
                "destination coordinate or node has a different Anchor",
            )),
            None => Ok(None),
        }
    }
}

fn conflict(message: &str) -> Error {
    Error::typed("concord.issue.transfer.conflict", message)
}
