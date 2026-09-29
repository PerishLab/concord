use super::{Integration, decode};
use crate::estate::{Coordinate, Estate, fault};
use crate::{Error, Result};

impl Estate {
    pub async fn integrations(&self) -> Result<Vec<Integration>> {
        let mut integrations = self
            .core
            .live("Integration")
            .await
            .map_err(fault)?
            .iter()
            .map(decode)
            .collect::<Result<Vec<_>>>()?;
        integrations.sort_by_key(|integration| integration.repository.identity());
        Ok(integrations)
    }

    pub async fn integration(&self, issue: &Coordinate) -> Result<Integration> {
        self.integrations()
            .await?
            .into_iter()
            .find(|integration| {
                integration.repository.owner == issue.owner
                    && integration.repository.name == issue.repository
            })
            .ok_or_else(|| {
                Error::typed(
                    "concord.integration.absent",
                    format!(
                        "repository integration is not registered: {}/{}",
                        issue.owner, issue.repository
                    ),
                )
            })
    }

    pub(in crate::estate) async fn locate(&self, node: &str) -> Result<Integration> {
        self.integrations()
            .await?
            .into_iter()
            .find(|integration| integration.node == node)
            .ok_or_else(|| {
                Error::typed(
                    "concord.integration.absent",
                    format!("repository node is not registered: {node}"),
                )
            })
    }
}
